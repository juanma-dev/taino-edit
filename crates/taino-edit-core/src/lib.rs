//! `taino-edit-core` — framework-agnostic heart of the taino-edit WYSIWYG editor.
//!
//! This crate has **zero** web, Leptos or Dioxus dependencies. It is the
//! reusable ~80% (document model, transforms, state, history, commands,
//! keymap, serializers) shared by every framework adapter.
//!
//! It is kept `#![no_std]`-friendly *where reasonable*; the document model in
//! Phase 1 will decide the final allocator/`alloc` boundary.
//!
//! Status: **pre-implementation** — see `ROADMAP.md` Phase 1.

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(missing_docs, rust_2018_idioms)]

// Modules land in Phase 1+ (document model → transforms → state → commands).
