//! [`Schema`] — the set of node and mark types a document may use — and the
//! [`SchemaBuilder`] used to declare one.
//!
//! v0.1 exposes only the builder API. A `schema! {}` macro is deferred to
//! v0.2 (it would be sugar over this builder, not a different architecture —
//! see DESIGN_NOTES §6).

use std::collections::HashMap;
use std::sync::Arc;

use crate::attrs::{AttrValue, Attrs};
use crate::content::{compile_content, ContentMatch};
use crate::error::{DocError, SchemaError};
use crate::mark::{Mark, MarkType, MarkTypeInner};
use crate::node::{Node, NodeType, NodeTypeInner};

/// Declaration of a single attribute: its default value, if any. An attribute
/// with no default is required at construction time.
#[derive(Debug, Clone, Default)]
pub struct AttrSpec {
    /// Default value used when the attribute is omitted.
    pub default: Option<AttrValue>,
}

/// Declaration of a node type.
#[derive(Debug, Clone, Default)]
pub struct NodeSpec {
    /// Content expression for valid children (e.g. `"paragraph+"`). `None`
    /// means a leaf node.
    pub content: Option<String>,
    /// Group(s) this node belongs to (space-separated), referenced from
    /// other nodes' content expressions.
    pub group: Option<String>,
    /// Mark expression for allowed marks: `None` = allow all on inline nodes
    /// and none on block nodes; `Some("_")` = allow all; `Some("")` = none;
    /// `Some("strong em")` = only those.
    pub marks: Option<String>,
    /// Whether this node is inline (text is always inline regardless).
    pub inline: bool,
    /// Whether this node is an opaque atom even if it has content.
    pub atom: bool,
    /// Attribute declarations.
    pub attrs: HashMap<String, AttrSpec>,
}

/// Declaration of a mark type.
#[derive(Debug, Clone, Default)]
pub struct MarkSpec {
    /// Group(s) this mark belongs to.
    pub group: Option<String>,
    /// Whether the mark is inclusive (the editor extends it when typing at
    /// its boundary). Stored for adapters; not interpreted by `core` in v0.1.
    pub inclusive: bool,
    /// Attribute declarations.
    pub attrs: HashMap<String, AttrSpec>,
}

/// Builder for a [`Schema`]. Node and mark declaration order is preserved and
/// determines schema ids.
#[derive(Default)]
pub struct SchemaBuilder {
    nodes: Vec<(String, NodeSpec)>,
    marks: Vec<(String, MarkSpec)>,
    top_node: Option<String>,
}

impl SchemaBuilder {
    /// Start an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a node type. The type named `text` is the document's text
    /// node and must be a leaf.
    pub fn node(mut self, name: &str, spec: NodeSpec) -> Self {
        self.nodes.push((name.to_string(), spec));
        self
    }

    /// Declare a mark type.
    pub fn mark(mut self, name: &str, spec: MarkSpec) -> Self {
        self.marks.push((name.to_string(), spec));
        self
    }

    /// Set the top (document) node type name. Defaults to `"doc"`.
    pub fn top_node(mut self, name: &str) -> Self {
        self.top_node = Some(name.to_string());
        self
    }

    /// Validate and compile the schema.
    pub fn build(self) -> Result<Schema, SchemaError> {
        if self.nodes.is_empty() {
            return Err(SchemaError::Empty);
        }

        // Assign ids; detect duplicates.
        let mut node_index: HashMap<String, usize> = HashMap::new();
        for (i, (name, _)) in self.nodes.iter().enumerate() {
            if node_index.insert(name.clone(), i).is_some() {
                return Err(SchemaError::DuplicateType(name.clone()));
            }
        }
        let mut mark_index: HashMap<String, usize> = HashMap::new();
        for (i, (name, _)) in self.marks.iter().enumerate() {
            if mark_index.insert(name.clone(), i).is_some() {
                return Err(SchemaError::DuplicateType(name.clone()));
            }
        }

        let top_name = self.top_node.clone().unwrap_or_else(|| "doc".to_string());
        if !node_index.contains_key(&top_name) {
            return Err(SchemaError::UnknownTopNode(top_name));
        }

        // Group → node ids, for content-expression resolution.
        let mut group_ids: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, (_, spec)) in self.nodes.iter().enumerate() {
            if let Some(g) = &spec.group {
                for grp in g.split_whitespace() {
                    group_ids.entry(grp.to_string()).or_default().push(i);
                }
            }
        }
        let name_to_id = node_index.clone();
        let resolve = move |name: &str| -> Option<Vec<usize>> {
            if let Some(&id) = name_to_id.get(name) {
                Some(vec![id])
            } else {
                group_ids.get(name).cloned()
            }
        };

        // Build node types.
        let mut nodes: Vec<NodeType> = Vec::with_capacity(self.nodes.len());
        let mut content_matches: Vec<ContentMatch> = Vec::with_capacity(self.nodes.len());
        for (i, (name, spec)) in self.nodes.iter().enumerate() {
            let expr = spec.content.clone().unwrap_or_default();
            let cm = compile_content(name, &expr, &resolve)?;
            // A node is a leaf when its content expression can match no
            // child of any type (e.g. `text`, an empty expression, an image).
            let is_leaf = (0..self.nodes.len()).all(|t| cm.match_type(t).is_none());
            let groups = spec
                .group
                .as_ref()
                .map(|g| g.split_whitespace().map(String::from).collect())
                .unwrap_or_default();
            nodes.push(NodeType(Arc::new(NodeTypeInner {
                id: i,
                name: name.clone(),
                spec: spec.clone(),
                groups,
                is_text: name == "text",
                content_is_empty: is_leaf,
            })));
            content_matches.push(cm);
        }

        let marks: Vec<MarkType> = self
            .marks
            .iter()
            .enumerate()
            .map(|(i, (name, spec))| {
                MarkType(Arc::new(MarkTypeInner {
                    id: i,
                    name: name.clone(),
                    spec: spec.clone(),
                }))
            })
            .collect();

        let top = node_index[&top_name];
        Ok(Schema(Arc::new(SchemaInner {
            nodes,
            node_index,
            marks,
            mark_index,
            top,
            content_matches,
        })))
    }
}

#[derive(Debug)]
pub(crate) struct SchemaInner {
    nodes: Vec<NodeType>,
    node_index: HashMap<String, usize>,
    marks: Vec<MarkType>,
    mark_index: HashMap<String, usize>,
    top: usize,
    content_matches: Vec<ContentMatch>,
}

/// An immutable, validated set of node and mark types. Cloning is O(1).
#[derive(Debug, Clone)]
pub struct Schema(pub(crate) Arc<SchemaInner>);

impl Schema {
    /// Look up a node type by name.
    pub fn node_type(&self, name: &str) -> Option<&NodeType> {
        self.0.node_index.get(name).map(|&i| &self.0.nodes[i])
    }

    /// Look up a mark type by name.
    pub fn mark_type(&self, name: &str) -> Option<&MarkType> {
        self.0.mark_index.get(name).map(|&i| &self.0.marks[i])
    }

    /// The top (document) node type.
    pub fn top_node_type(&self) -> &NodeType {
        &self.0.nodes[self.0.top]
    }

    /// All node types, ordered by id.
    pub fn node_types(&self) -> &[NodeType] {
        &self.0.nodes
    }

    /// All mark types, ordered by id.
    pub fn mark_types(&self) -> &[MarkType] {
        &self.0.marks
    }

    pub(crate) fn content_match(&self, type_id: usize) -> &ContentMatch {
        &self.0.content_matches[type_id]
    }

    fn fill_attrs(spec_attrs: &HashMap<String, AttrSpec>, mut given: Attrs) -> Attrs {
        for (k, s) in spec_attrs {
            if !given.contains_key(k) {
                if let Some(d) = &s.default {
                    given.insert(k.clone(), d.clone());
                }
            }
        }
        given
    }

    /// Whether `children` (in order) satisfy `parent`'s content expression.
    pub fn content_valid(&self, parent: &NodeType, children: &[Node]) -> bool {
        let cm = self.content_match(parent.id());
        cm.matches_complete(children.iter().map(|c| c.node_type().id()))
    }

    /// Build a validated element node.
    ///
    /// Attributes are filled from spec defaults. Returns
    /// [`DocError::UnknownNodeType`] for an unknown name or
    /// [`DocError::InvalidContent`] if the children violate the schema.
    pub fn node(
        &self,
        name: &str,
        attrs: Attrs,
        children: Vec<Node>,
        marks: Vec<Mark>,
    ) -> Result<Node, DocError> {
        let nt = self
            .node_type(name)
            .ok_or_else(|| DocError::UnknownNodeType(name.to_string()))?
            .clone();
        if !self.content_valid(&nt, &children) {
            return Err(DocError::InvalidContent {
                parent: name.to_string(),
            });
        }
        let attrs = Self::fill_attrs(&nt.0.spec.attrs, attrs);
        Ok(Node::new_element(
            nt,
            attrs,
            crate::fragment::Fragment::from_nodes(children),
            marks,
        ))
    }

    /// Build a text node carrying `text` with the given marks.
    pub fn text(&self, text: &str, marks: Vec<Mark>) -> Result<Node, DocError> {
        let nt = self
            .0
            .nodes
            .iter()
            .find(|n| n.is_text())
            .ok_or_else(|| DocError::UnknownNodeType("text".to_string()))?
            .clone();
        Ok(Node::new_text(nt, text.to_string(), marks))
    }
}
