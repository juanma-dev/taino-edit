# taino-edit

Native Rust rich text editor (WYSIWYG) framework for Leptos and Dioxus, in the same `taino-*` family as the author's prior `taino-dnd-*` contributions.

**Read [DESIGN_NOTES.md](DESIGN_NOTES.md) first.** It contains the full architecture, scope decisions, LOC budget, naming rationale, and open questions that came out of the design conversation. This file is just the entry-point pointer.

## Project status

Pre-implementation. No code yet. The next step after design is deciding the open questions in §6 of DESIGN_NOTES.md (which framework adapter ships first, document model style, schema DSL, etc.) and then scaffolding the workspace.

## Author context

- This is the author's **second and final planned contribution** to the Rust ecosystem for the time being (after `taino-dnd-*`). Scope discipline is therefore important — see DESIGN_NOTES §3 for the deliberately narrow v0.1 cut.
- Author works on Windows (Windows 11, bash + PowerShell available). Repo lives at `c:\JM\PROGRAMMING\taino-edit`.

## Working agreements

- Pure Rust at runtime. No JS bridges in the published crates (the whole point — `leptos-tiptap` already exists as a JS wrapper and isn't what we're building).
- Framework-agnostic `core` is sacred: it must not depend on `web-sys`, `leptos`, or `dioxus`.
- LOC estimates and time budgets in DESIGN_NOTES are honest and binding — push back if scope creeps beyond them.
