//! `taino-edit-extensions` — the v0.1 extension set for taino-edit.
//!
//! Each extension is a self-contained module exposing `schema_additions()`,
//! `commands()` and `keymap_entries()`. The v0.1 cut is: `bold`, `italic`,
//! `heading`, `paragraph`, `history`.
//!
//! Status: **pre-implementation** — see `ROADMAP.md` Phase 6.

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(missing_docs, rust_2018_idioms)]

// Extension modules land in Phase 6, on top of the Phase 1-3 core.
