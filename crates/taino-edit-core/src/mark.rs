//! Inline marks (bold, italic, link, …) and their schema-bound types.

use std::sync::Arc;

use crate::attrs::Attrs;
use crate::schema::MarkSpec;

#[derive(Debug)]
pub(crate) struct MarkTypeInner {
    pub(crate) id: usize,
    pub(crate) name: String,
    pub(crate) spec: MarkSpec,
}

/// A schema-bound mark type. Cheap to clone (an [`Arc`] handle); identity is
/// by schema-assigned id.
#[derive(Debug, Clone)]
pub struct MarkType(pub(crate) Arc<MarkTypeInner>);

impl MarkType {
    /// The mark type's unique name within its schema.
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// The schema-assigned id (stable for the lifetime of the schema).
    pub fn id(&self) -> usize {
        self.0.id
    }

    /// The spec this type was built from.
    pub fn spec(&self) -> &MarkSpec {
        &self.0.spec
    }

    /// Instantiate a mark of this type with the given attributes.
    pub fn create(&self, attrs: Attrs) -> Mark {
        Mark {
            type_: self.clone(),
            attrs,
        }
    }
}

impl PartialEq for MarkType {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}
impl Eq for MarkType {}

/// An applied inline annotation: a [`MarkType`] plus its attributes.
#[derive(Debug, Clone)]
pub struct Mark {
    pub(crate) type_: MarkType,
    pub(crate) attrs: Attrs,
}

impl Mark {
    /// The type of this mark.
    pub fn mark_type(&self) -> &MarkType {
        &self.type_
    }

    /// This mark's attributes.
    pub fn attrs(&self) -> &Attrs {
        &self.attrs
    }

    /// Whether this mark is in `set` (same type and attributes).
    pub fn is_in_set(&self, set: &[Mark]) -> bool {
        set.iter().any(|m| m == self)
    }

    /// Return `set` with this mark added. If a mark of the same type is
    /// present it is replaced. The result stays sorted by mark type id so
    /// serialization is deterministic.
    pub fn add_to_set(&self, set: &[Mark]) -> Vec<Mark> {
        let mut out: Vec<Mark> = set
            .iter()
            .filter(|m| m.type_ != self.type_)
            .cloned()
            .collect();
        out.push(self.clone());
        out.sort_by_key(|m| m.type_.0.id);
        out
    }

    /// Return `set` with any mark of this mark's type removed.
    pub fn remove_from_set(&self, set: &[Mark]) -> Vec<Mark> {
        set.iter().filter(|m| *m != self).cloned().collect()
    }
}

impl PartialEq for Mark {
    fn eq(&self, other: &Self) -> bool {
        self.type_ == other.type_ && self.attrs == other.attrs
    }
}
impl Eq for Mark {}

/// Whether two mark sets are equal as sets (order-insensitive).
pub fn same_mark_set(a: &[Mark], b: &[Mark]) -> bool {
    a.len() == b.len() && a.iter().all(|m| m.is_in_set(b))
}
