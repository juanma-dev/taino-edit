//! [`Node`] — a single element (or text run) in the document tree — and
//! [`NodeType`], its schema-bound descriptor.

use std::sync::Arc;

use crate::attrs::Attrs;
use crate::fragment::Fragment;
use crate::mark::Mark;
use crate::schema::NodeSpec;

#[derive(Debug)]
pub(crate) struct NodeTypeInner {
    pub(crate) id: usize,
    pub(crate) name: String,
    pub(crate) spec: NodeSpec,
    pub(crate) groups: Vec<String>,
    pub(crate) is_text: bool,
    /// `true` when the compiled content expression accepts no children.
    pub(crate) content_is_empty: bool,
}

/// A schema-bound node type. Cheap to clone; identity is by schema id.
#[derive(Debug, Clone)]
pub struct NodeType(pub(crate) Arc<NodeTypeInner>);

impl NodeType {
    /// The type's unique name within its schema.
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// The schema-assigned id (stable for the lifetime of the schema).
    pub fn id(&self) -> usize {
        self.0.id
    }

    /// The spec this type was built from.
    pub fn spec(&self) -> &NodeSpec {
        &self.0.spec
    }

    /// Whether this is the text node type.
    pub fn is_text(&self) -> bool {
        self.0.is_text
    }

    /// Whether this type is inline (text, or a spec marked `inline`).
    pub fn is_inline(&self) -> bool {
        self.0.is_text || self.0.spec.inline
    }

    /// Whether this type is a block (the negation of [`is_inline`]).
    ///
    /// [`is_inline`]: NodeType::is_inline
    pub fn is_block(&self) -> bool {
        !self.is_inline()
    }

    /// Whether this type never has content (a leaf such as an image or
    /// horizontal rule, or any node whose content expression is empty).
    pub fn is_leaf(&self) -> bool {
        self.0.content_is_empty
    }

    /// Whether this type is treated as a single opaque unit (`atom` in the
    /// spec, or any leaf).
    pub fn is_atom(&self) -> bool {
        self.0.spec.atom || self.is_leaf()
    }

    /// Whether this type belongs to content group `group`.
    pub fn is_in_group(&self, group: &str) -> bool {
        self.0.groups.iter().any(|g| g == group)
    }
}

impl PartialEq for NodeType {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}
impl Eq for NodeType {}

#[derive(Debug)]
pub(crate) struct NodeInner {
    pub(crate) type_: NodeType,
    pub(crate) attrs: Attrs,
    pub(crate) content: Fragment,
    pub(crate) marks: Vec<Mark>,
    /// `Some` only for text nodes.
    pub(crate) text: Option<String>,
}

/// A node in the document tree: an element with attributes, child content and
/// marks, or — when [`is_text`](Node::is_text) — a marked text run.
///
/// Positions follow the ProseMirror model. A text node's size is its length
/// in Unicode scalar values (`char`s — note this differs from ProseMirror's
/// UTF-16 units; the DOM bridge maps between them). A non-text leaf has size
/// 1; any other node has size `content.size + 2`.
#[derive(Debug, Clone)]
pub struct Node(pub(crate) Arc<NodeInner>);

impl Node {
    pub(crate) fn new_element(
        type_: NodeType,
        attrs: Attrs,
        content: Fragment,
        marks: Vec<Mark>,
    ) -> Node {
        Node(Arc::new(NodeInner {
            type_,
            attrs,
            content,
            marks,
            text: None,
        }))
    }

    pub(crate) fn new_text(type_: NodeType, text: String, marks: Vec<Mark>) -> Node {
        Node(Arc::new(NodeInner {
            type_,
            attrs: Attrs::new(),
            content: Fragment::empty(),
            marks,
            text: Some(text),
        }))
    }

    /// This node's type.
    pub fn node_type(&self) -> &NodeType {
        &self.0.type_
    }

    /// This node's attributes.
    pub fn attrs(&self) -> &Attrs {
        &self.0.attrs
    }

    /// This node's child fragment (empty for text and leaf nodes).
    pub fn content(&self) -> &Fragment {
        &self.0.content
    }

    /// The marks applied to this node.
    pub fn marks(&self) -> &[Mark] {
        &self.0.marks
    }

    /// The text of a text node, or `None` for element nodes.
    pub fn text(&self) -> Option<&str> {
        self.0.text.as_deref()
    }

    /// Whether this is a text node.
    pub fn is_text(&self) -> bool {
        self.0.text.is_some()
    }

    /// Whether this node is a leaf (no content).
    pub fn is_leaf(&self) -> bool {
        self.0.type_.is_leaf()
    }

    /// Whether this node is inline.
    pub fn is_inline(&self) -> bool {
        self.0.type_.is_inline()
    }

    /// Whether this node is a block.
    pub fn is_block(&self) -> bool {
        self.0.type_.is_block()
    }

    /// Number of direct children.
    pub fn child_count(&self) -> usize {
        self.0.content.child_count()
    }

    /// Borrow the child at index `i`.
    ///
    /// # Panics
    /// Panics if `i >= child_count()`.
    pub fn child(&self, i: usize) -> &Node {
        self.0.content.child(i)
    }

    /// The number of positions this node occupies in its parent.
    pub fn node_size(&self) -> usize {
        if let Some(t) = &self.0.text {
            t.chars().count()
        } else if self.0.type_.is_leaf() {
            1
        } else {
            self.0.content.size() + 2
        }
    }

    /// The concatenated text of this node and its descendants.
    pub fn text_content(&self) -> String {
        if let Some(t) = &self.0.text {
            return t.clone();
        }
        let mut s = String::new();
        for child in self.0.content.iter() {
            s.push_str(&child.text_content());
        }
        s
    }

    /// Return a copy of this node with `marks` as its mark set.
    pub fn with_marks(&self, marks: Vec<Mark>) -> Node {
        let inner = &*self.0;
        Node(Arc::new(NodeInner {
            type_: inner.type_.clone(),
            attrs: inner.attrs.clone(),
            content: inner.content.clone(),
            marks,
            text: inner.text.clone(),
        }))
    }

    /// Return a copy of this node with the same type/attrs/marks but the
    /// given content. Not schema-validated — callers that need validation
    /// (e.g. the replace algorithm) check separately.
    pub(crate) fn copy_content(&self, content: Fragment) -> Node {
        debug_assert!(self.0.text.is_none(), "copy_content on a text node");
        let inner = &*self.0;
        Node(Arc::new(NodeInner {
            type_: inner.type_.clone(),
            attrs: inner.attrs.clone(),
            content,
            marks: inner.marks.clone(),
            text: None,
        }))
    }

    /// Return a copy of this text node carrying `text`.
    pub(crate) fn with_text(&self, text: String) -> Node {
        debug_assert!(self.0.text.is_some(), "with_text on a non-text node");
        Node::new_text(self.0.type_.clone(), text, self.0.marks.clone())
    }

    /// Whether two nodes have the same type, attributes and marks (text
    /// equality aside) — i.e. adjacent text runs can be merged.
    pub(crate) fn same_markup(&self, other: &Node) -> bool {
        self.0.type_ == other.0.type_
            && self.0.attrs == other.0.attrs
            && self.0.marks == other.0.marks
    }

    /// Attributes (mutable-copy helper for `AttrStep`): return a copy with
    /// `attrs` replacing the current attribute map.
    #[allow(dead_code)] // consumed by AttrStep, landing later this phase
    pub(crate) fn with_attrs(&self, attrs: Attrs) -> Node {
        let inner = &*self.0;
        Node(Arc::new(NodeInner {
            type_: inner.type_.clone(),
            attrs,
            content: inner.content.clone(),
            marks: inner.marks.clone(),
            text: inner.text.clone(),
        }))
    }

    /// Slice this node's content (text for text nodes) between content
    /// positions `from..to`, returning a same-markup copy.
    pub(crate) fn cut(&self, from: usize, to: usize) -> Node {
        if let Some(t) = &self.0.text {
            let chars: Vec<char> = t.chars().collect();
            let to = to.min(chars.len());
            if from == 0 && to == chars.len() {
                return self.clone();
            }
            return self.with_text(chars[from..to].iter().collect());
        }
        if from == 0 && to == self.0.content.size() {
            return self.clone();
        }
        self.copy_content(self.0.content.cut(from, to))
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        self.0.type_ == other.0.type_
            && self.0.text == other.0.text
            && self.0.attrs == other.0.attrs
            && self.0.marks == other.0.marks
            && self.0.content == other.0.content
    }
}
impl Eq for Node {}
