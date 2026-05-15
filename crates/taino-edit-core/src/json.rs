//! JSON (de)serialization of documents.
//!
//! The wire format mirrors ProseMirror's: an object with `type`, optional
//! `attrs`, optional `content` (array), optional `marks` (array), and `text`
//! for text nodes. Deserialization is **schema-checked** — content that
//! violates the parent's content expression is rejected — so a document
//! round-trips through JSON without loss.

use serde_json::{Map, Value};

use crate::attrs::Attrs;
use crate::error::DocError;
use crate::mark::Mark;
use crate::node::Node;
use crate::schema::Schema;

impl Mark {
    /// Serialize this mark to JSON.
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("type".into(), Value::String(self.mark_type().name().into()));
        if !self.attrs().is_empty() {
            obj.insert(
                "attrs".into(),
                Value::Object(self.attrs().clone().into_iter().collect()),
            );
        }
        Value::Object(obj)
    }
}

impl Node {
    /// Serialize this node (and its subtree) to JSON.
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("type".into(), Value::String(self.node_type().name().into()));
        if let Some(text) = self.text() {
            obj.insert("text".into(), Value::String(text.into()));
        }
        if !self.attrs().is_empty() {
            obj.insert(
                "attrs".into(),
                Value::Object(self.attrs().clone().into_iter().collect()),
            );
        }
        if self.child_count() > 0 {
            let content: Vec<Value> = self.content().iter().map(Node::to_json).collect();
            obj.insert("content".into(), Value::Array(content));
        }
        if !self.marks().is_empty() {
            let marks: Vec<Value> = self.marks().iter().map(Mark::to_json).collect();
            obj.insert("marks".into(), Value::Array(marks));
        }
        Value::Object(obj)
    }
}

fn obj<'a>(v: &'a Value, ctx: &str) -> Result<&'a Map<String, Value>, DocError> {
    v.as_object()
        .ok_or_else(|| DocError::MalformedJson(format!("{ctx}: expected an object")))
}

fn attrs_from(v: Option<&Value>) -> Result<Attrs, DocError> {
    match v {
        None => Ok(Attrs::new()),
        Some(Value::Object(m)) => Ok(m.clone().into_iter().collect()),
        Some(_) => Err(DocError::MalformedJson("`attrs` must be an object".into())),
    }
}

impl Schema {
    /// Parse a mark from JSON, validating its type against the schema.
    pub fn mark_from_json(&self, v: &Value) -> Result<Mark, DocError> {
        let m = obj(v, "mark")?;
        let name = m
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| DocError::MalformedJson("mark: missing `type`".into()))?;
        let mt = self
            .mark_type(name)
            .ok_or_else(|| DocError::UnknownMarkType(name.to_string()))?
            .clone();
        Ok(mt.create(attrs_from(m.get("attrs"))?))
    }

    fn marks_from(&self, v: Option<&Value>) -> Result<Vec<Mark>, DocError> {
        match v {
            None => Ok(Vec::new()),
            Some(Value::Array(a)) => a.iter().map(|m| self.mark_from_json(m)).collect(),
            Some(_) => Err(DocError::MalformedJson("`marks` must be an array".into())),
        }
    }

    /// Parse a node (and its subtree) from JSON, validating every node
    /// against the schema (unknown types and invalid content are rejected).
    pub fn node_from_json(&self, v: &Value) -> Result<Node, DocError> {
        let m = obj(v, "node")?;
        let name = m
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| DocError::MalformedJson("node: missing `type`".into()))?;
        let marks = self.marks_from(m.get("marks"))?;

        if let Some(text) = m.get("text") {
            let text = text
                .as_str()
                .ok_or_else(|| DocError::MalformedJson("`text` must be a string".into()))?;
            return self.text(text, marks);
        }

        let children = match m.get("content") {
            None => Vec::new(),
            Some(Value::Array(a)) => a
                .iter()
                .map(|c| self.node_from_json(c))
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(DocError::MalformedJson("`content` must be an array".into())),
        };
        self.node(name, attrs_from(m.get("attrs"))?, children, marks)
    }

    /// Parse a document from a JSON string.
    pub fn parse_json_str(&self, s: &str) -> Result<Node, DocError> {
        let v: Value =
            serde_json::from_str(s).map_err(|e| DocError::MalformedJson(e.to_string()))?;
        self.node_from_json(&v)
    }
}
