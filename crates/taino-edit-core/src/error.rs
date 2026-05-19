//! Error types for schema construction and document operations.

use std::fmt;

/// Error raised while building or using a [`Schema`](crate::Schema).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    /// A node or mark name was declared more than once.
    DuplicateType(String),
    /// The declared top node name has no matching node type.
    UnknownTopNode(String),
    /// A content expression referenced an unknown node name or group.
    UnknownContentRef {
        /// The node type whose content expression is invalid.
        in_type: String,
        /// The unresolved name or group token.
        reference: String,
    },
    /// A content expression failed to parse.
    BadContentExpression {
        /// The node type whose content expression is invalid.
        in_type: String,
        /// Human-readable parser message.
        message: String,
    },
    /// No node type was registered at all.
    Empty,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::DuplicateType(n) => write!(f, "duplicate type name `{n}`"),
            SchemaError::UnknownTopNode(n) => {
                write!(f, "top node `{n}` is not a declared node type")
            }
            SchemaError::UnknownContentRef { in_type, reference } => {
                write!(f, "content of `{in_type}` references unknown `{reference}`")
            }
            SchemaError::BadContentExpression { in_type, message } => {
                write!(f, "content expression of `{in_type}` is invalid: {message}")
            }
            SchemaError::Empty => write!(f, "schema declares no node types"),
        }
    }
}

impl std::error::Error for SchemaError {}

/// Error raised while constructing or deserializing a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocError {
    /// A referenced node type name is not in the schema.
    UnknownNodeType(String),
    /// A referenced mark type name is not in the schema.
    UnknownMarkType(String),
    /// Children did not satisfy the parent node type's content expression.
    InvalidContent {
        /// Parent node type name.
        parent: String,
    },
    /// A position was out of range for the document it was resolved against.
    PositionOutOfRange {
        /// The offending position.
        pos: usize,
        /// The maximum valid position.
        max: usize,
    },
    /// JSON did not match the expected document shape.
    MalformedJson(String),
    /// HTML input was malformed or exceeded a safety limit (e.g. nesting
    /// depth).
    HtmlParse(String),
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocError::UnknownNodeType(n) => write!(f, "unknown node type `{n}`"),
            DocError::UnknownMarkType(n) => write!(f, "unknown mark type `{n}`"),
            DocError::InvalidContent { parent } => {
                write!(f, "content does not match the schema for `{parent}`")
            }
            DocError::PositionOutOfRange { pos, max } => {
                write!(f, "position {pos} out of range (max {max})")
            }
            DocError::MalformedJson(m) => write!(f, "malformed document JSON: {m}"),
            DocError::HtmlParse(m) => write!(f, "could not parse HTML: {m}"),
        }
    }
}

impl std::error::Error for DocError {}
