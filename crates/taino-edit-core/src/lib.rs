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
mod commands;
mod content;
mod error;
mod fragment;
mod html;
mod inputrules;
mod json;
mod keymap;
mod map;
mod mark;
pub mod markdown;
mod node;
mod plugin;
mod pos;
mod replace;
mod schema;
pub mod schema_macro;
mod selection;
mod slice;
mod state;
mod step;
mod transform;

pub use attrs::{AttrValue, Attrs};
pub use commands::{
    caret_left, caret_line_end, caret_line_start, caret_right, chain, delete_backward,
    delete_forward, delete_selection, join_backward, join_forward, lift, remove_mark, select_all,
    set_block_type, set_mark, split_block, toggle_mark, wrap_in, Command, Dispatch,
};
pub use content::ContentMatch;
pub use error::{DocError, SchemaError};
pub use fragment::Fragment;
pub use html::{DomSpec, HtmlElement, ParseRule, MAX_DEPTH};
pub use inputrules::{
    text_replace_rule, textblock_type_rule, wrapping_rule, InputRule, InputRules,
};
pub use keymap::{base_keymap, KeyPress, Keymap};
pub use map::{MapResult, Mapping, StepMap, DEL_SIDE};
pub use mark::{same_mark_set, Mark, MarkType};
pub use node::{Node, NodeType};
pub use plugin::{Plugin, PluginKey, PluginSet};
pub use pos::ResolvedPos;
#[doc(no_inline)]
pub use regex::Captures;
pub use replace::ReplaceError;
pub use schema::{AttrSpec, MarkSpec, NodeSpec, Schema, SchemaBuilder};
pub use selection::Selection;
pub use slice::Slice;
pub use state::{EditorState, History, HistoryIntent, Transaction};
pub use step::{
    step_from_json, AddMarkStep, AttrStep, RemoveMarkStep, ReplaceAroundStep, ReplaceStep, Step,
    StepError,
};
pub use transform::Transform;
