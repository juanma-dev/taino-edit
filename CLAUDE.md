# taino-edit

Native Rust rich text editor (WYSIWYG) framework for Leptos and Dioxus, in the same `taino-*` family as the author's prior `taino-dnd-*` contributions.

**Read [DESIGN_NOTES.md](DESIGN_NOTES.md) and [ROADMAP.md](ROADMAP.md) first.** DESIGN_NOTES contains the architecture, scope budget, and resolved design decisions. ROADMAP contains the phased v0.1 plan, current status, and contribution surfaces. This file is just the entry-point pointer.

## Project status

Pre-implementation. All design decisions in DESIGN_NOTES §6 are resolved. Next step: **Phase 0** in [ROADMAP.md](ROADMAP.md) — workspace scaffold and CI baseline.

## Author context

- This is the author's **second and final planned contribution** to the Rust ecosystem for the time being (after `taino-dnd-*`). Scope discipline is therefore important — see DESIGN_NOTES §3 for the deliberately narrow v0.1 cut.
- Author works on **WSL Ubuntu on Windows 11**. Rust toolchain and git are installed inside WSL. The repo lives at `~/projects/taino-edit` (WSL native filesystem, accessible from Windows as `\\wsl.localhost\Ubuntu\home\juanma\projects\taino-edit`). All `cargo`, `git`, and toolchain commands should be run from WSL.

## Working agreements

- Pure Rust at runtime. No JS bridges in the published crates (the whole point — `leptos-tiptap` already exists as a JS wrapper and isn't what we're building).
- Framework-agnostic `core` is sacred: it must not depend on `web-sys`, `leptos`, or `dioxus`.
- LOC estimates and time budgets in DESIGN_NOTES are honest and binding — push back if scope creeps beyond them.
