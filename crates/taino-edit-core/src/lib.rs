//! `taino-edit-core` — framework-agnostic heart of the taino-edit WYSIWYG
//! editor.
//!
//! This crate has **zero** web, Leptos or Dioxus dependencies. It is the
//! reusable model shared by every framework adapter: a ProseMirror-style
//! tree of typed [`Node`]s and [`Mark`]s, validated against a [`Schema`]
//! built with [`SchemaBuilder`], with schema-checked JSON (de)serialization
//! and absolute-position resolution ([`ResolvedPos`]).
//!
//! ```
//! use taino_edit_core::{SchemaBuilder, NodeSpec};
//!
//! let schema = SchemaBuilder::new()
//!     .node("doc", NodeSpec { content: Some("paragraph+".into()), ..Default::default() })
//!     .node("paragraph", NodeSpec { content: Some("text*".into()), ..Default::default() })
//!     .node("text", NodeSpec::default())
//!     .top_node("doc")
//!     .build()
//!     .unwrap();
//!
//! let hello = schema.text("hello", vec![]).unwrap();
//! let para = schema.node("paragraph", Default::default(), vec![hello], vec![]).unwrap();
//! let doc = schema.node("doc", Default::default(), vec![para], vec![]).unwrap();
//! assert_eq!(doc.text_content(), "hello");
//! ```
//!
//! Status: Phase 1 (document model). See `ROADMAP.md`.

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(missing_docs, rust_2018_idioms)]

mod attrs;
mod content;
mod error;
mod fragment;
mod html;
mod json;
mod mark;
mod node;
mod pos;
mod replace;
mod schema;
mod slice;

pub use attrs::{AttrValue, Attrs};
pub use content::ContentMatch;
pub use error::{DocError, SchemaError};
pub use fragment::Fragment;
pub use html::{DomSpec, HtmlElement, ParseRule, MAX_DEPTH};
pub use mark::{same_mark_set, Mark, MarkType};
pub use node::{Node, NodeType};
pub use pos::ResolvedPos;
pub use replace::ReplaceError;
pub use schema::{AttrSpec, MarkSpec, NodeSpec, Schema, SchemaBuilder};
pub use slice::Slice;
