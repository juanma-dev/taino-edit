//! `taino-edit-dom` — the contenteditable/DOM bridge for taino-edit.
//!
//! Renders a [`taino_edit_core`] document into a `contenteditable` element,
//! observes user edits via `MutationObserver`, and keeps the browser
//! selection in sync with the core [`Selection`](taino_edit_core::Selection).
//!
//! This is the only crate in the workspace where `unsafe` is permitted, and
//! only at the `wasm-bindgen`/`web-sys` FFI boundary. Each occurrence must
//! carry an inline `// SAFETY:` justification.
//!
//! Status: Phase 4 (DOM bridge). See `ROADMAP.md`.

#![warn(missing_docs, rust_2018_idioms)]

pub mod desc;
pub mod view;

pub use desc::ViewDesc;
pub use view::EditorView;
