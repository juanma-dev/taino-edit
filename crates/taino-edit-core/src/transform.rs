//! [`Transform`] — an accumulating sequence of [`Step`]s plus the combined
//! position [`Mapping`], with ergonomic editing helpers.
//!
//! Every applied step records the document it produced and appends its map,
//! so positions taken before a change can be mapped forward and the whole
//! transform can be inverted step-by-step (the basis for undo).

use crate::map::Mapping;
use crate::mark::Mark;
use crate::node::Node;
use crate::schema::Schema;
use crate::slice::Slice;
use crate::step::{AddMarkStep, AttrStep, RemoveMarkStep, ReplaceStep, Step, StepError};

/// An accumulating, invertible batch of document changes.
#[derive(Debug, Clone)]
pub struct Transform {
    doc: Node,
    steps: Vec<Box<dyn Step>>,
    docs: Vec<Node>,
    mapping: Mapping,
}

impl Transform {
    /// Start a transform from `doc`.
    pub fn new(doc: Node) -> Self {
        Transform {
            doc,
            steps: Vec::new(),
            docs: Vec::new(),
            mapping: Mapping::new(),
        }
    }

    /// The current document (after the steps applied so far).
    pub fn doc(&self) -> &Node {
        &self.doc
    }

    /// The steps applied so far.
    pub fn steps(&self) -> &[Box<dyn Step>] {
        &self.steps
    }

    /// The document *before* step `i`.
    pub fn doc_before(&self, i: usize) -> &Node {
        &self.docs[i]
    }

    /// The combined position mapping for all steps so far.
    pub fn mapping(&self) -> &Mapping {
        &self.mapping
    }

    /// Whether any step has been applied.
    pub fn doc_changed(&self) -> bool {
        !self.steps.is_empty()
    }

    /// Apply `step`, recording the prior document and its map.
    pub fn step(&mut self, step: Box<dyn Step>, schema: &Schema) -> Result<&mut Self, StepError> {
        let next = step.apply(&self.doc, schema)?;
        self.mapping.append_map(step.get_map());
        self.docs.push(std::mem::replace(&mut self.doc, next));
        self.steps.push(step);
        Ok(self)
    }

    /// Replace `from..to` with `slice`.
    pub fn replace(
        &mut self,
        from: usize,
        to: usize,
        slice: Slice,
        schema: &Schema,
    ) -> Result<&mut Self, StepError> {
        self.step(Box::new(ReplaceStep::new(from, to, slice)), schema)
    }

    /// Delete `from..to`.
    pub fn delete(
        &mut self,
        from: usize,
        to: usize,
        schema: &Schema,
    ) -> Result<&mut Self, StepError> {
        self.replace(from, to, Slice::empty(), schema)
    }

    /// Insert `slice` at `pos`.
    pub fn insert(
        &mut self,
        pos: usize,
        slice: Slice,
        schema: &Schema,
    ) -> Result<&mut Self, StepError> {
        self.replace(pos, pos, slice, schema)
    }

    /// Add `mark` across `from..to`.
    pub fn add_mark(
        &mut self,
        from: usize,
        to: usize,
        mark: Mark,
        schema: &Schema,
    ) -> Result<&mut Self, StepError> {
        self.step(Box::new(AddMarkStep::new(from, to, mark)), schema)
    }

    /// Remove `mark` across `from..to`.
    pub fn remove_mark(
        &mut self,
        from: usize,
        to: usize,
        mark: Mark,
        schema: &Schema,
    ) -> Result<&mut Self, StepError> {
        self.step(Box::new(RemoveMarkStep::new(from, to, mark)), schema)
    }

    /// Set attribute `attr` to `value` on the node at `pos`.
    pub fn set_node_attr(
        &mut self,
        pos: usize,
        attr: &str,
        value: serde_json::Value,
        schema: &Schema,
    ) -> Result<&mut Self, StepError> {
        self.step(Box::new(AttrStep::new(pos, attr, value)), schema)
    }

    /// Invert every step (last-first) against the document it applied to,
    /// yielding the steps that undo this whole transform.
    pub fn invert_steps(&self) -> Result<Vec<Box<dyn Step>>, StepError> {
        let mut inverted = Vec::with_capacity(self.steps.len());
        for i in (0..self.steps.len()).rev() {
            inverted.push(self.steps[i].invert(&self.docs[i])?);
        }
        Ok(inverted)
    }
}
