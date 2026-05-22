//! `taino-edit` — umbrella crate for the taino-edit WYSIWYG editor framework.
//!
//! Native Rust, reactive-first rich-text editing. **No JavaScript bridge** —
//! unlike `leptos-tiptap`, this is pure Rust at runtime.
//!
//! This crate re-exports the workspace pieces behind feature flags so
//! consumers pick exactly what they need:
//!
//! | Feature      | Re-exports                                            |
//! |--------------|-------------------------------------------------------|
//! | *(always)*   | [`core`] — document model, transforms, state, commands |
//! | `extensions` | [`extensions`] — bold, italic, heading, link, image, lists, tables, … |
//! | `dom`        | [`dom`] — the contenteditable bridge + `ViewPlugin`   |
//! | `leptos`     | [`leptos`] — the Leptos adapter (implies `dom`+`extensions`) |
//! | `dioxus`     | [`dioxus`] — the Dioxus adapter                       |
//! | `table-view` | [`table_view`] — table pointer interaction (cell drag-select, resize) |
//!
//! No adapter is enabled by default; choose one, e.g.
//! `taino-edit = { version = "0.3", features = ["leptos"] }`.

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(rust_2018_idioms)]

pub use taino_edit_core as core;

#[cfg(feature = "extensions")]
pub use taino_edit_extensions as extensions;

#[cfg(feature = "dom")]
pub use taino_edit_dom as dom;

#[cfg(feature = "leptos")]
pub use taino_edit_leptos as leptos;

#[cfg(feature = "dioxus")]
pub use taino_edit_dioxus as dioxus;

#[cfg(feature = "table-view")]
pub use taino_edit_table_view as table_view;
