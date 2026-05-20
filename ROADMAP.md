# taino-edit — Roadmap

> Native Rust WYSIWYG editor framework for Leptos (and, post-v0.1, Dioxus).
> See [DESIGN_NOTES.md](DESIGN_NOTES.md) for architecture, scope rationale, and resolved design decisions.

This document is the single source of truth for **what has been done, what is in progress, and what is planned**. It is updated as work lands.

---

## Status at a glance

|                              |                                                          |
| ---------------------------- | -------------------------------------------------------- |
| **Current phase**            | 5 — `taino-edit-leptos` adapter (Phase 4 done)           |
| **Last updated**             | 2026-05-15                                               |
| **First milestone**          | `v0.1.0` — publishable MVP                               |
| **Effort estimate to v0.1**  | 2–4 months full-time solo (~10–14k LOC, excluding tests) |
| **First framework adapter**  | Leptos                                                   |
| **License**                  | MIT OR Apache-2.0                                        |
| **MSRV**                     | 1.80                                                     |

### Progress legend

- ✅ Done
- 🚧 In progress
- ⏳ Planned for v0.1
- 💤 Deferred to v0.2+
- ❄️ Out of scope for v0.1 — community contribution surface

---

## Snapshot

### Done

- ✅ Market research — confirmed gap in pure-Rust WYSIWYG for Leptos and Dioxus (see [DESIGN_NOTES §1](DESIGN_NOTES.md#1-why-this-project-exists))
- ✅ Layered architecture — `core` / `dom` / framework adapter / extensions
- ✅ LOC and time budget — v0.1 ~10–14k LOC, v1.0 ~20–26k LOC
- ✅ Crate naming — `taino-edit` family of 6 crates
- ✅ All design decisions in [DESIGN_NOTES §6](DESIGN_NOTES.md#6-resolved-decisions-2026-05-15) resolved
- ✅ Locked technical choices: MSRV, Leptos pinning strategy, CI matrix, license
- ✅ **Phase 0 — Workspace scaffold and CI baseline** (2026-05-15): six crates build/`fmt`/`clippy`/`test`/`doc` green; `cargo package --workspace` verifies all six; Leptos pinned at `0.8`, `web-sys`/`js-sys` `0.3`, `wasm-bindgen` `0.2`
- ✅ **Phase 1 — Core: document model** (2026-05-19): typed tree (Node/Mark/Fragment/Slice), schema + content automaton, `ResolvedPos`, schema-checked JSON round-trip, and a dependency-free escaped HTML serializer + strict depth-bounded HTML parser; 14 acceptance tests in `taino-edit-core`
- ✅ **Phase 2 — Core: transforms, state, history** (2026-05-19): ProseMirror-ported `replace`, all five steps (Replace/ReplaceAround/AddMark/RemoveMark/Attr) with invert+map+JSON, StepMap/Mapping with mirror-recover, `Transform`, `Selection`, `EditorState`/`Transaction`, bounded undo/redo `History`; 29 step/transform/state tests (generic plugin registry deferred to v0.2)
- ✅ **Phase 3 — Core: commands, keymap, input rules** (2026-05-19): `Command`/`chain`, selection/mark/block/join commands, `Transform::split`, cross-platform `Keymap` + `base_keymap`, regex `InputRules` (`text_replace`/`textblock_type`/`wrapping`); 27 command/keymap/inputrule tests. `taino-edit-core` is feature-complete for v0.1
- ✅ **Phase 4 — `taino-edit-dom`: the contenteditable bridge** (2026-05-20): `EditorView` with mount + incremental DOM diff/patch; bidirectional `Selection` ↔ `getSelection`; `read_dom_changes()` for typing; IME composition lifecycle; clipboard `paste_text`/`paste_html` (HTML sanitized through `Schema::parse_html`); drag/drop primitives; focus + tabindex; node decorations. **46 wasm_bindgen_test cases pass in headless Chromium 148** via a small patch on `wasm-bindgen-cli` (vendored in `vendor/`) + `scripts/wasm-test.sh`. Adapter-side event wiring (MutationObserver, selectionchange, paste/drop, composition events) lands in Phase 5

### In progress

- 🚧 *(nothing yet — Phase 5 begins next session)*

### Up next

- ⏳ **Phase 5 — `taino-edit-leptos` adapter** (target: ~1 week)

---

## Phases

Phases are sequential. Each ends in a state where `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo doc --no-deps` are all green — so the project is publishable (even if functionally incomplete) at every checkpoint.

### Phase 0 — Workspace scaffold and CI baseline
**Goal:** six crates compile, CI green, repository is contribution-ready.
**Effort:** ~1 week.
**Definition of done:** `cargo publish --dry-run` succeeds for every crate.
✅ Met via `cargo package --workspace` (all six `.crate`s build + verify).
Note: per-crate `cargo publish --dry-run` only passes for the dep-free crates
(`core`, `dioxus`); the others can't resolve unpublished workspace siblings
against crates.io until Phase 7's ordered publish — a known cargo limitation,
not a scaffold defect. `cargo package --workspace` is the correct pre-publish
gate and it is green.

- [x] Initialize `git` repo, `.gitignore` for Rust + WASM artifacts
- [x] `rust-toolchain.toml` pinning channel `stable` and MSRV `1.80`
- [x] Top-level `Cargo.toml` workspace listing `crates/*`
- [x] Create six crate skeletons under `crates/`:
  - [x] `taino-edit-core` — `#![no_std]`-friendly where reasonable, zero web deps
  - [x] `taino-edit-dom` — `web-sys`, `wasm-bindgen`, `js-sys`
  - [x] `taino-edit-extensions` — depends on `core`
  - [x] `taino-edit-leptos` — depends on `core` + `dom` + `leptos`
  - [x] `taino-edit-dioxus` — empty placeholder, `#![doc = "Reserved for v0.2"]`
  - [x] `taino-edit` — umbrella crate, re-exports gated by features (`leptos`, `dioxus`, `dom`)
- [x] `LICENSE-MIT` + `LICENSE-APACHE` at repo root; `license = "MIT OR Apache-2.0"` and `repository`, `keywords`, `categories` in every `Cargo.toml` (via `[workspace.package]` inheritance). `documentation` deliberately **omitted** — one inherited URL would be wrong for every sub-crate; crates.io auto-links the correct per-crate docs.rs page
- [x] `README.md` — pitch, status warning, install snippet, links to design docs
- [x] `CONTRIBUTING.md` — build/test commands, PR conventions, code-of-conduct link
- [x] `CHANGELOG.md` — Keep-a-Changelog format with `## [Unreleased]`
- [x] `.github/workflows/ci.yml` running on push and PR:
  - [x] `cargo fmt --all -- --check`
  - [x] `cargo clippy --all-targets --all-features -- -D warnings`
  - [x] `cargo test --all-features`
  - [x] `cargo doc --no-deps --all-features`
- [x] `.github/dependabot.yml` for monthly cargo and actions updates
- [x] Issue templates: bug, feature, RFC
- [x] `deny.toml` for `cargo-deny` (advisories, licenses, bans)

### Phase 1 — Core: document model
**Goal:** a typed, traversable, serializable document tree.
**Effort:** 2–3 weeks. Estimated LOC: ~1.5–2k.
**Definition of done:** schema-validated documents round-trip through JSON without loss; unit tests cover traversal edge cases.

- [x] `Node` — element with type, attributes, content (`Fragment`), marks
- [x] `Mark` — inline annotation with type and attributes
- [x] `Fragment` — ordered, immutable sequence of `Node`s
- [x] `Slice` — fragment with open depths for cut-paste boundaries
- [x] `NodeType`, `MarkType` — schema-bound type descriptors
- [x] `Schema` + `SchemaBuilder` — the builder API (no macro yet)
- [x] Content expressions — minimal regex-like grammar for valid children (`"paragraph+"`, `"(text | image)*"`) — Thompson NFA → DFA, ProseMirror-compatible
- [x] `Pos` (absolute) and `ResolvedPos` (path + parent context)
- [x] `serde::Serialize`/`Deserialize` for documents → JSON (schema-checked, round-trips without loss)
- [x] HTML serializer (one-way: doc → HTML string) — escaped output, schema-driven `to_dom`
- [x] HTML parser (HTML string → doc), strict against schema — dependency-free tokenizer, depth-bounded, hostile-input-safe
- [x] Snapshot tests for all of the above — `tests/model.rs` (JSON round-trip + traversal) and `tests/html.rs` (round-trip, escaping, strictness, hostile input)

### Phase 2 — Core: transforms, state, history
**Goal:** mutate documents through validated, invertible steps; persist editor state and selection.
**Effort:** 2–3 weeks. Estimated LOC: ~2.5–3k.
**Definition of done:** undo/redo correct across all step types; transform-against-step contract documented even if no concurrent path uses it yet.

- [x] `Step` trait — `apply`, `invert`, `map(&Mapping)`, `to_json`, `from_json`. Designed to support a future `map_against(&Step)` for CRDT/OT integration (documented on the trait).
- [x] Concrete steps:
  - [x] `ReplaceStep`
  - [x] `ReplaceAroundStep`
  - [x] `AddMarkStep`
  - [x] `RemoveMarkStep`
  - [x] `AttrStep`
- [x] `Mapping` — composable position remap across multiple steps (StepMap + mirror/recover)
- [x] `Transform` — fluent builder that accumulates steps + their mapping
- [x] `Selection` enum — `Text`, `Node`, `All` (positional mapping; "valid selection nearby" is a v0.2 refinement)
- [x] `EditorState` — doc + selection + schema + history
- [x] `Transaction` — `Transform` + selection updates + history intent
- [~] `Plugin` trait + `PluginKey` — **v0.1 cut**: history is the one built-in stateful component; the generic typed-plugin registry is deferred to v0.2 (see Deferred)
- [x] `History` plugin — bounded undo/redo stack with caller-driven grouping

### Phase 3 — Core: commands, keymap, input rules
**Goal:** the standard editing vocabulary that every WYSIWYG needs.
**Effort:** ~1 week. Estimated LOC: ~1–1.5k.
**Definition of done:** all baseKeymap commands have tests; toggleMark/setBlockType behave on selections of every shape.

- [x] `Command` type — `Fn(&EditorState, Option<&mut Dispatch>) -> bool` + `chain`
- [x] Selection-level commands: `delete_selection`, `select_all`
- [x] Mark commands: `toggle_mark`, `set_mark`, `remove_mark`
- [x] Block commands: `set_block_type`, `wrap_in`, `lift`, `split_block` (+ `Transform::split`)
- [x] Join commands: `join_backward`, `join_forward`
- [x] `Keymap` with cross-platform modifier handling (Mod = Ctrl on Win/Linux, Cmd on macOS)
- [x] `base_keymap` — Enter, Backspace, Delete, arrows, Home/End (+ `delete_backward`/`forward`, caret motion)
- [x] `InputRules` — regex-triggered transforms: `text_replace_rule`, `textblock_type_rule` (`## ` → heading), `wrapping_rule` (`> ` → blockquote)

### Phase 4 — `taino-edit-dom`: the contenteditable bridge
**Goal:** render `EditorState` to the DOM, observe user edits, sync selection.
**Effort:** 2–3 weeks. Estimated LOC: ~2–3k. **This is the riskiest phase** — contenteditable is famously hostile.
**Definition of done:** a manual harness can type, select, apply marks, undo, paste plain text, and IME-compose without state desync, in Chromium and Firefox.

- [~] `NodeView` trait — pluggable per-node-type rendering via `NodeSpec.to_dom`/`MarkSpec.to_dom` (from `core`); a richer per-node-view trait with imperative DOM hooks is deferred to v0.2
- [x] Default `ViewDesc` tree — mirrors document tree, holds DOM nodes
- [x] DOM diff/patch — minimal mutations on state change (text-only reuses the same DOM text node; identical subtrees untouched)
- [~] `MutationObserver` adapter — `EditorView::read_dom_changes()` produces a `Transform` from DOM-side text edits; wiring an actual `MutationObserver` and dispatching is the adapter's job (Phase 5)
- [x] Selection sync — `window.getSelection()` ↔ core `Selection` (`set_selection`/`read_selection`, bidirectional)
- [~] Reentrancy guard — IME `composing` flag covers the most acute case; a `selectionchange` echo guard ships with the Phase 5 adapter event wiring
- [x] IME composition — `composition_start`/`composition_end`; `read_dom_changes` suppresses transient glyph states
- [x] Clipboard — `paste_text` and `paste_html` (the latter sanitized through `Schema::parse_html`; copy/cut are adapter-driven serialization of selection)
- [x] Drag-and-drop — `extract_slice` + `drop_slice` (the actual `dragstart`/`drop` event wiring and `DataTransfer` serialization is adapter-side)
- [x] Focus management and `tabindex` — `focus`/`has_focus`/`set_tabindex`; mount sets `tabindex="0"` by default
- [~] Decorations — node-level (`Decoration::Node` adds a CSS class to a block); inline range decorations deferred to v0.2 (require text-node splitting that interacts with diff/patch)

### Phase 5 — `taino-edit-leptos` adapter
**Goal:** an idiomatic Leptos component.
**Effort:** ~1 week. Estimated LOC: ~0.5–1k.
**Definition of done:** the example app builds with `trunk serve` and renders a working editor.

- [ ] `<TainoEditor>` component with props: `state: Signal<EditorState>`, `dispatch: Callback<Transaction>`, `plugins: Vec<Plugin>`
- [ ] Mount/unmount lifecycle — `create_effect` to attach `dom` view, `on_cleanup` to tear down
- [ ] Bridge between Leptos signals and `dom`-layer state pushes (avoid double-update loops)
- [ ] Public re-exports: schema builder, common commands, `base_keymap`
- [ ] Storybook-style example pages under `examples/leptos/`

### Phase 6 — `taino-edit-extensions`: the v0.1 extension set
**Goal:** five extensions that prove the architecture and make the demo non-trivial.
**Effort:** ~1 week. Estimated LOC: ~0.5–1k.
**Definition of done:** each extension is a single module exposing a `schema_additions()`, `commands()`, and `keymap_entries()`.

- [ ] `bold` — `Mod-b`
- [ ] `italic` — `Mod-i`
- [ ] `heading` — h1/h2/h3 with `Mod-Alt-1..3`
- [ ] `paragraph` — base block, `Mod-Alt-0`
- [ ] `history` — re-exports core history plugin with default keymap (`Mod-z`, `Mod-Shift-z`)

### Phase 7 — Polish and `v0.1.0` release
**Goal:** publish.
**Effort:** 1–2 weeks.
**Definition of done:** all 6 crates on crates.io, docs.rs builds clean, `examples/basic-leptos` runs from a fresh checkout.

- [ ] `examples/basic-leptos` — full editor demo using `cargo-leptos`/`trunk`
- [ ] `examples/headless-core` — server-side document manipulation, no DOM
- [ ] Crate-level rustdoc: every public item documented, examples in `core`
- [ ] `README.md` upgrade with screenshot/GIF and feature checklist
- [ ] `CHANGELOG.md` v0.1.0 entry
- [ ] Publish to crates.io in dependency order: `core` → `extensions` → `dom` → `leptos` → umbrella
- [ ] Tag `v0.1.0`, GitHub Release with highlights and known limitations
- [ ] Announce: r/rust, This Week in Rust, Leptos Discord

---

## Deferred (v0.2+)

- 💤 Generic `Plugin` trait + `PluginKey` typed-state registry — v0.1 ships `History` as the one built-in stateful component; the multi-plugin dispatch generalisation lands here
- 💤 `schema!{}` proc-macro DSL — sugar over the v0.1 builder
- 💤 `taino-edit-dioxus` adapter — same dom layer, different reactivity bridge
- 💤 `loro` integration behind `collab` feature — collaborative editing via Peritext CRDT
- 💤 Markdown serializer + parser
- 💤 Richer extensions: lists, links, images, code blocks, blockquotes, tables (basic)
- 💤 `Decoration` API for third-party inline UI (mentions, comments)
- 💤 Server-side rendering of the initial document (Leptos SSR)

## Out of scope for v0.1 (community contributions welcome)

- ❄️ Real-time collaborative cursors / presence
- ❄️ `taino-edit-blitz` or `taino-edit-freya` native (non-DOM) renderers
- ❄️ Mobile touch gestures beyond the contenteditable defaults
- ❄️ Full WCAG audit (basic accessibility yes; certified audit no)
- ❄️ Mentions, autocomplete, slash menus
- ❄️ Track changes, comments, suggestion mode

---

## Risk register

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| Contenteditable cross-browser bugs (especially Safari) | High | High | Phase 4 includes a manual test matrix; Safari support may be marked "best effort" in v0.1 |
| IME composition correctness for CJK input | Medium | High | Dedicated composition lifecycle in Phase 4; recruit testers from Discord before release |
| Selection-sync feedback loops between MutationObserver and our own writes | High | Medium | Reentrancy guard built in from day one; integration tests with synthetic mutations |
| Performance on documents >10k nodes | Medium | Medium | v0.1 targets typical-sized docs; rope-based text storage deferred to v0.2 if needed |
| `Step` design forecloses CRDT integration | Low | High | `map_against(&Step)` contract documented in Phase 2 even though no caller uses it yet |
| Scope creep past LOC budget | Medium | Medium | This roadmap is the gate; new work goes into "Deferred" unless it unblocks v0.1 |

---

## Working conventions

- **Branching:** trunk-based on `main`; topic branches for anything non-trivial.
- **Commits:** Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`).
- **PRs:** every PR ticks at least one checkbox in this file or moves an item between sections.
- **Reviews:** solo project for now; treat the CI matrix as the reviewer that never sleeps.
- **Issue triage:** label by phase (`phase-0` … `phase-7`) and crate (`crate:core`, etc.).

## How to read this file

This document **lags** the code by zero commits — if `main` says feature X is shipped, this file must say so too. PRs that change behavior should update the relevant phase checklist in the same commit.
