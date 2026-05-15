//! `taino-edit-dom` — the contenteditable/DOM bridge for taino-edit.
//!
//! Renders a [`taino_edit_core`] `EditorState` to the DOM, observes user
//! edits via `MutationObserver`, and keeps the browser selection in sync with
//! the core selection.
//!
//! This is the only crate in the workspace where `unsafe` is permitted, and
//! only at the `wasm-bindgen`/`web-sys` FFI boundary — every occurrence must
//! carry an inline `// SAFETY:` justification.
//!
//! Status: **pre-implementation** — see `ROADMAP.md` Phase 4.

#![warn(missing_docs, rust_2018_idioms)]

// `web-sys` / `wasm-bindgen` / `js-sys` wiring lands in Phase 4.
