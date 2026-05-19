# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Pre-1.0, minor version bumps may include breaking API changes.

## [Unreleased]

### Added

- Phase 0 — Cargo workspace scaffold and CI baseline:
  - Six-crate workspace: `taino-edit-core`, `taino-edit-dom`,
    `taino-edit-extensions`, `taino-edit-leptos`, `taino-edit-dioxus`
    (v0.2 placeholder), and the `taino-edit` umbrella crate.
  - `rust-toolchain.toml` (stable, MSRV 1.80, `wasm32-unknown-unknown`).
  - Dual `MIT OR Apache-2.0` licensing.
  - GitHub Actions CI: `fmt`, `clippy`, `test`, `doc`.
  - Dependabot, issue templates, and `cargo-deny` configuration.
- Phase 1 — `taino-edit-core` document model:
  - ProseMirror-style typed tree: `Node`/`NodeType`, `Mark`/`MarkType`
    (with mark-set operations), `Fragment`, `Slice`.
  - `Schema` + `SchemaBuilder` with attribute defaults and content
    validation; content expressions compiled via a Thompson NFA → DFA
    (`paragraph+`, `(text | image)*`, `+ * ?`).
  - `ResolvedPos` absolute-position resolution
    (`depth`/`start`/`end`/`before`/`after`/`text_offset`).
  - Schema-checked JSON (de)serialization that round-trips without loss.
  - Dependency-free HTML serializer (escaped output) and a strict,
    depth-bounded HTML parser validated against the schema.

[Unreleased]: https://github.com/juanma-dev/taino-edit/commits/main
