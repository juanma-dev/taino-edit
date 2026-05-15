//! Node and mark attribute values.
//!
//! Attribute values reuse [`serde_json::Value`] so documents round-trip
//! through JSON without a bespoke value type, and attribute maps are ordered
//! ([`BTreeMap`]) so serialization is deterministic (important for snapshot
//! tests).

use std::collections::BTreeMap;

/// A single attribute value. Any JSON-representable value is permitted.
pub type AttrValue = serde_json::Value;

/// An ordered map of attribute name to [`AttrValue`].
pub type Attrs = BTreeMap<String, AttrValue>;
