//! `taino-edit-leptos` — the Leptos adapter for taino-edit.
//!
//! A thin reactive bridge: a `<TainoEditor>` component that maps Leptos
//! `Signal`s onto a [`taino_edit_core`] `EditorState` and drives the
//! [`taino_edit_dom`] view. The Leptos reactive surface (`Signal`, `Effect`,
//! `view!`) is deliberately isolated here so framework version bumps stay
//! contained.
//!
//! Leptos is the first-class adapter for v0.1 (Dioxus is reserved for v0.2).
//!
//! Status: **pre-implementation** — see `ROADMAP.md` Phase 5.

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(missing_docs, rust_2018_idioms)]

// `<TainoEditor>` component lands in Phase 5.
