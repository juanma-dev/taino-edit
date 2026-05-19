//! [`Step`] — an atomic, invertible, mappable document change — and the
//! v0.1 concrete steps. Steps are the unit of change history and the
//! designed-in extension point for future OT/CRDT integration.

use std::fmt;

use serde_json::{json, Value};

use crate::error::DocError;
use crate::fragment::Fragment;
use crate::map::{Mapping, StepMap};
use crate::node::Node;
use crate::schema::Schema;
use crate::slice::Slice;

/// A step failed to apply to the given document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepError(pub String);

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "step failed: {}", self.0)
    }
}
impl std::error::Error for StepError {}

/// An atomic document change.
///
/// Every step can be **applied**, **inverted** (given the document it was
/// applied to) and **mapped** through a [`Mapping`] so concurrent or
/// rebased changes compose. `to_json` plus
/// [`step_from_json`] give lossless persistence. A future `map_against`
/// for CRDT/OT can be added without reshaping this trait (DESIGN_NOTES §6).
pub trait Step: fmt::Debug {
    /// Apply the step, returning the new document or why it failed.
    fn apply(&self, doc: &Node, schema: &Schema) -> Result<Node, StepError>;

    /// How this step remaps positions.
    fn get_map(&self) -> StepMap;

    /// The step that undoes this one, given the document it applied to.
    fn invert(&self, doc: &Node) -> Result<Box<dyn Step>, StepError>;

    /// This step rebased through `mapping`, or `None` if it is entirely
    /// mapped away (its whole range was deleted).
    fn map(&self, mapping: &Mapping) -> Option<Box<dyn Step>>;

    /// Serialize to JSON (tagged with `stepType`).
    fn to_json(&self) -> Value;
}

fn slice_to_json(slice: &Slice) -> Value {
    if slice.is_empty() {
        return json!({});
    }
    let content: Vec<Value> = slice.content().iter().map(Node::to_json).collect();
    json!({
        "content": content,
        "openStart": slice.open_start(),
        "openEnd": slice.open_end(),
    })
}

fn slice_from_json(schema: &Schema, v: &Value) -> Result<Slice, DocError> {
    let obj = v
        .as_object()
        .ok_or_else(|| DocError::MalformedJson("slice must be an object".into()))?;
    let content = match obj.get("content") {
        None => return Ok(Slice::empty()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|n| schema.node_from_json(n))
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(DocError::MalformedJson(
                "slice.content must be an array".into(),
            ))
        }
    };
    let open_start = obj.get("openStart").and_then(Value::as_u64).unwrap_or(0) as usize;
    let open_end = obj.get("openEnd").and_then(Value::as_u64).unwrap_or(0) as usize;
    Ok(Slice::new(
        Fragment::from_nodes(content),
        open_start,
        open_end,
    ))
}

/// Replace the range `from..to` with `slice`.
#[derive(Debug, Clone)]
pub struct ReplaceStep {
    from: usize,
    to: usize,
    slice: Slice,
}

impl ReplaceStep {
    /// A step replacing `from..to` with `slice`.
    pub fn new(from: usize, to: usize, slice: Slice) -> Self {
        ReplaceStep { from, to, slice }
    }
}

impl Step for ReplaceStep {
    fn apply(&self, doc: &Node, schema: &Schema) -> Result<Node, StepError> {
        doc.replace(self.from, self.to, &self.slice, schema)
            .map_err(|e| StepError(e.to_string()))
    }

    fn get_map(&self) -> StepMap {
        StepMap::new(vec![self.from, self.to - self.from, self.slice.size()])
    }

    fn invert(&self, doc: &Node) -> Result<Box<dyn Step>, StepError> {
        let removed = doc
            .slice(self.from, self.to)
            .map_err(|e| StepError(e.to_string()))?;
        Ok(Box::new(ReplaceStep {
            from: self.from,
            to: self.from + self.slice.size(),
            slice: removed,
        }))
    }

    fn map(&self, mapping: &Mapping) -> Option<Box<dyn Step>> {
        let from = mapping.map_result(self.from, 1);
        let to = mapping.map_result(self.to, -1);
        if from.deleted_across() && to.deleted_across() {
            return None;
        }
        Some(Box::new(ReplaceStep {
            from: from.pos,
            to: from.pos.max(to.pos),
            slice: self.slice.clone(),
        }))
    }

    fn to_json(&self) -> Value {
        json!({
            "stepType": "replace",
            "from": self.from,
            "to": self.to,
            "slice": slice_to_json(&self.slice),
        })
    }
}

/// Reconstruct a step from its JSON form (produced by [`Step::to_json`]).
pub fn step_from_json(schema: &Schema, v: &Value) -> Result<Box<dyn Step>, DocError> {
    let obj = v
        .as_object()
        .ok_or_else(|| DocError::MalformedJson("step must be an object".into()))?;
    let kind = obj
        .get("stepType")
        .and_then(Value::as_str)
        .ok_or_else(|| DocError::MalformedJson("step missing `stepType`".into()))?;
    match kind {
        "replace" => {
            let from = obj
                .get("from")
                .and_then(Value::as_u64)
                .ok_or_else(|| DocError::MalformedJson("replace step missing `from`".into()))?
                as usize;
            let to = obj
                .get("to")
                .and_then(Value::as_u64)
                .ok_or_else(|| DocError::MalformedJson("replace step missing `to`".into()))?
                as usize;
            let slice = match obj.get("slice") {
                Some(s) => slice_from_json(schema, s)?,
                None => Slice::empty(),
            };
            Ok(Box::new(ReplaceStep::new(from, to, slice)))
        }
        other => Err(DocError::MalformedJson(format!(
            "unknown stepType `{other}`"
        ))),
    }
}
