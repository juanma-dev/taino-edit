# taino-edit

> Native Rust WYSIWYG rich-text editor framework for [Leptos](https://leptos.dev) — pure Rust at runtime, **no JavaScript bridge**.

[![CI](https://github.com/juanma-dev/taino-edit/actions/workflows/ci.yml/badge.svg)](https://github.com/juanma-dev/taino-edit/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

`taino-edit` is a ProseMirror/TipTap-inspired editor — typed document model,
invertible transforms, plugins, history — built reactive-first for Rust web
frameworks. Unlike `leptos-tiptap` (a `wasm-bindgen` wrapper around the
TypeScript TipTap bundle), there is **no JS dependency at runtime**.

It is part of the `taino-*` family, following `taino-dnd-*`.

## ⚠️ Status: pre-implementation

This repository is currently a **workspace scaffold only** (Phase 0). No
editing functionality exists yet. The architecture and the phased v0.1 plan
are fully specified — see the design docs below. Do not depend on this crate
until `v0.1.0` is published.

- **[DESIGN_NOTES.md](DESIGN_NOTES.md)** — architecture, scope budget, resolved design decisions
- **[ROADMAP.md](ROADMAP.md)** — phased v0.1 plan, current status, contribution surfaces

## Workspace layout

| Crate                                                  | Role                                                              |
| ------------------------------------------------------ | ----------------------------------------------------------------- |
| [`taino-edit-core`](crates/taino-edit-core)             | Framework-agnostic model, transforms, state, history, commands    |
| [`taino-edit-dom`](crates/taino-edit-dom)               | `contenteditable`/DOM bridge (`web-sys`, `wasm-bindgen`, `js-sys`) |
| [`taino-edit-extensions`](crates/taino-edit-extensions) | Bold, italic, heading, paragraph, history                         |
| [`taino-edit-leptos`](crates/taino-edit-leptos)         | Leptos adapter (first-class for v0.1)                              |
| [`taino-edit-dioxus`](crates/taino-edit-dioxus)         | Placeholder, reserved for v0.2                                     |
| [`taino-edit`](crates/taino-edit)                       | Umbrella crate, feature-gated re-exports                           |

## Install (once published)

```toml
[dependencies]
taino-edit = { version = "0.1", features = ["leptos"] }
```

No adapter is enabled by default — choose `leptos` (or, post-v0.1, `dioxus`).

## Build & test

Requires the Rust toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml)
(stable, MSRV 1.80).

```sh
cargo check --workspace
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The roadmap marks community
contribution surfaces (the Dioxus adapter, richer extensions, native
renderers) explicitly.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual-licensed as above, without any additional
terms or conditions.
