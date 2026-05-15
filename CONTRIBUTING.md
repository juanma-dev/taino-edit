# Contributing to taino-edit

Thanks for your interest! This is a solo-maintained project with a
deliberately narrow v0.1 scope — please read
[DESIGN_NOTES.md](DESIGN_NOTES.md) and [ROADMAP.md](ROADMAP.md) before opening
non-trivial PRs. The roadmap's *Deferred* and *Out of scope* sections mark
where community contributions are most welcome.

## Prerequisites

- Rust toolchain as pinned in [`rust-toolchain.toml`](rust-toolchain.toml)
  (stable channel, MSRV **1.80**). `rustup` will install it automatically.
- The `wasm32-unknown-unknown` target (declared in the toolchain file).

## Build & test

Every change must keep all four of these green — CI enforces them:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

The project is publishable at every checkpoint, so also keep
`cargo publish --dry-run` succeeding for each crate.

## Commit & PR conventions

- **Branching:** trunk-based on `main`; topic branches for anything non-trivial.
- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/)
  — `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
- **PRs:** every PR should tick at least one checkbox in
  [ROADMAP.md](ROADMAP.md) or move an item between sections, and update the
  relevant phase checklist **in the same PR** as the behavior change.
- **Issues:** label by phase (`phase-0` … `phase-7`) and crate
  (`crate:core`, `crate:dom`, …).

## Scope discipline

LOC and time budgets in [DESIGN_NOTES.md](DESIGN_NOTES.md) §3 are honest and
binding. New work that pushes past them goes into *Deferred* unless it
unblocks v0.1. Pushing back on scope creep is encouraged, not discouraged.

## Architectural invariants

- Pure Rust at runtime — **no JS bridge** in published crates.
- `taino-edit-core` must not depend on `web-sys`, `leptos`, or `dioxus`.
- `#![deny(unsafe_code)]` everywhere except `taino-edit-dom`, where any
  `unsafe` at the FFI boundary must carry an inline `// SAFETY:` comment.

## Code of conduct

This project follows the
[Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).
By participating you are expected to uphold it.
