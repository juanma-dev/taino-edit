# taino-edit — Roadmap

> Native Rust WYSIWYG editor framework for Leptos (and, post-v0.1, Dioxus).
> See [DESIGN_NOTES.md](DESIGN_NOTES.md) for architecture, scope rationale, and resolved design decisions.

This document is the single source of truth for **what has been done, what is in progress, and what is planned**. It is updated as work lands.

---

## Status at a glance

|                              |                                                          |
| ---------------------------- | -------------------------------------------------------- |
| **Current release**          | `v0.5.0` — `schema!` macro + inline (range-level) decorations + editing fixes |
| **Last updated**             | 2026-05-27                                               |
| **First milestone**          | `v0.1.0` — publishable MVP (done)                        |
| **Second milestone**         | `v0.2.0` — closing v0.1 gaps + platform broadening (done)|
| **Third milestone**          | `v0.3.0` — full tables + pointer-interaction platform (done)|
| **Effort estimate to v0.1**  | 2–4 months full-time solo (~11–15k LOC, excluding tests) |
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
- ✅ **Phase 5 — `taino-edit-leptos` adapter** (2026-05-20): the `<TainoEditor>` component backed by a `RwSignal<EditorState>`; mounts `EditorView` on first reactive tick, patches in place on every change, and wires `input`/`compositionstart`/`compositionend`/`paste` so browser-side edits commit back through the same signal. `examples/basic-leptos/` is `trunk serve`-buildable (Bold/Undo/Redo + editor). 6 wasm_bindgen_test cases run the component through Leptos's CSR runtime in headless Chromium
- ✅ **Phase 6 — `taino-edit-extensions`: the v0.1 extension set** (2026-05-21): `Extension` trait + `SchemaAdditions` + `build_schema_with`/`build_keymap_with` helpers. Initially five built-ins (`Bold`, `Italic`, `Heading`, `Paragraph`, `History`) shipped through Phase 6. Then **broadened mid-Phase-7** (2026-05-21) to **12 extensions** so the published v0.1 is something the community can drop into a real project without first writing four extensions themselves: added `Link` (with `set_link`/`remove_link`), `Image` (inline atom), `Align` (text_align on paragraph/heading), `TransformCase` (upper/lower), `Blockquote`, `CodeBlock`, and `Lists` (bullet/ordered + list_item with wrap + lift). All built on the existing schema/command/keymap surface — no `core` API changes were needed for the broader cut. 41 extension tests + a keymap improvement (shift-implicit lookup for symbol keys, so `Mod->` matches Ctrl+Shift+>) covered by their own host tests
- ✅ **Phase 7 — Polish for v0.1.0** (2026-05-21, code portion): `examples/headless-core` proves the core runs server-side; README rewritten with feature checklist + Leptos usage example; CHANGELOG `[0.1.0]` entry with explicit "Highlights" and "Known limitations" sections. All four CI gates green, **110 host tests + 52 wasm-bindgen-test cases** pass. Also fixed a latent UX bug found while dog-fooding the demo: `<TainoEditor>` now listens for `document.selectionchange` and mirrors the browser selection into `state.selection`, so toolbar buttons that depend on the caret (H1/H2/H3, align, set_link, …) act on the right block. Only the maintainer-only release steps remain (publish to crates.io, tag, announce)

### In progress

- 🚧 *(nothing yet — release is the maintainer's hand-off)*

### Up next

- ⏳ **Release `v0.1.0`** — `cargo publish` in dependency order, tag `v0.1.0`, post the GitHub Release and the announcements

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

- [x] `<TainoEditor>` component — final prop shape is a single `RwSignal<EditorState>` (cleaner than separate state + dispatch + plugins, since plugins are deferred to v0.2). Browser-side edits are committed back through the same signal.
- [x] Mount/unmount lifecycle — `Effect::new` mounts the `EditorView` on first reactive run and patches it on every state change; `on_cleanup` drops the view + detaches every event listener
- [x] Bridge between Leptos signals and `dom`-layer state pushes — `EditorView` is kept in a `StoredValue<…, LocalStorage>` so the (Send+Sync) effect closures can reach the `!Send` view through a Copy handle without echo loops
- [x] Public re-exports — `taino_edit_leptos::{SchemaBuilder, NodeSpec, MarkSpec, EditorState, Selection, Transaction, base_keymap, toggle_mark/set_mark/remove_mark, set_block_type/wrap_in/lift/split_block/join_…, EditorView, Decoration, …}`
- [~] Example pages under `examples/` — `examples/basic-leptos/` is the v0.1 cut (Bold/Undo/Redo + editor, `trunk serve`-buildable, DoD met). A richer storybook-style multi-page suite is polish for the release-prep pass

### Phase 6 — `taino-edit-extensions`: the v0.1 extension set
**Goal:** a complete-enough extension set that the published v0.1 is something you can drop into a real project.
**Effort:** ~1–2 weeks. Estimated LOC: ~2–3k.
**Definition of done:** each extension is a single module exposing a `schema_additions()` and `keymap_entries()`, with host tests covering schema additions and the keymap dispatch.

- [x] `Extension` trait + `SchemaAdditions` aggregation type + `build_schema_with` / `build_keymap_with` helpers
- [x] `bold` — `Mod-b` (`strong` mark, parses `<strong>` and `<b>`)
- [x] `italic` — `Mod-i` (`em` mark, parses `<em>` and `<i>`)
- [x] `heading` — h1/h2/h3 with `Mod-Alt-1..3` (`level` attr; parse `h1`/`h2`/`h3`)
- [x] `paragraph` — base block, `Mod-Alt-0` (`<p>`)
- [x] `history` — `Mod-z` / `Mod-Shift-z`. Threads through the standard `Command`/`Transaction` pipeline by tagging a `Transaction` with a new `HistoryIntent` that `EditorState::apply` recognises (and resolves to `undo`/`redo` instead of pushing a new history entry)
- [x] `link` — `<a href title>` mark with `set_link(href, title?)` and `remove_link` commands; no default keymap (the host wires `Mod-k` to a URL prompt)
- [x] `image` — inline `<img>` atom with `src`/`alt`/`title` attrs and an `insert_image(src, alt?)` command
- [x] `align` — `text_align` attribute on `paragraph` and `heading`, with `align_left/center/right/justify` commands bound to `Mod-Shift-{l,e,r,j}`; serializes as `style="text-align: …"`
- [x] `transform_case` — selection-scoped `to_uppercase` / `to_lowercase` commands; no default binding (case shortcuts collide with too many browser/window-manager bindings to ship a default)
- [x] `blockquote` — `<blockquote>` wrapper bound to `Mod->` (uses the new shift-implicit keymap lookup so symbol-key bindings work on US keyboards)
- [x] `code_block` — `<pre>` block bound to `` Mod-` ``
- [x] `lists` — `bullet_list`/`ordered_list`/`list_item` nodes plus `wrap_in_bullet_list` (`Mod-Shift-8`), `wrap_in_ordered_list` (`Mod-Shift-7`), and `lift_list_item` (`Shift-Tab`). Smart Enter and nested-list indent (sink) are deferred to v0.2 — they need a multi-level `split_at_depth` step the v0.1 budget couldn't absorb

### Phase 7 — Polish and `v0.1.0` release
**Goal:** publish.
**Effort:** 1–2 weeks.
**Definition of done:** all 6 crates on crates.io, docs.rs builds clean, `examples/basic-leptos` runs from a fresh checkout.

- [x] `examples/basic-leptos` — full editor demo using `trunk` (built in Phase 5)
- [x] `examples/headless-core` — server-side document manipulation, no DOM
- [x] Crate-level rustdoc: every public item documented (`#![warn(missing_docs)]` + CI `cargo doc -D warnings` enforce it); `core`, `dom`, `leptos`, and `extensions` each carry a working module-level example
- [~] `README.md` upgrade — feature checklist + usage example landed; a screenshot/GIF is the one remaining nice-to-have (live demo is in `examples/basic-leptos`)
- [x] `CHANGELOG.md` v0.1.0 entry (with `Highlights` and `Known limitations / explicitly deferred to v0.2` sections)
- [x] **Publish to crates.io** in dependency order — done 2026-05-21 (6 crates, v0.1.0)
- [x] **Tag `v0.1.0`** + GitHub Release pointing at the CHANGELOG — done 2026-05-21
- [~] **Announce** — TWiR / r/rust / Leptos Discord pending the maintainer (drafts staged)

---

## v0.2 — Shipped 2026-05-21

**Goal:** close the visible v0.1 gaps + broaden the platform so third parties can ship richer extensions without forking `core`.

### Phase 1 — List UX completion ✅

- [x] `split_list_item` command (multi-depth split: paragraph + list_item)
- [x] `sink_list_item` command (Tab to indent, nesting the current item into the previous sibling)
- [x] Multi-item `lift_list_item` preserves surviving siblings (single-item case still works)
- [x] Smart Enter (chain: split → lift-if-empty) and Tab wired in the `Lists` keymap
- [x] New `Transform::split_at_depth(pos, levels, schema)` helper

### Phase 2 — Plugin trait + PluginKey ✅

- [x] `Plugin` trait with associated `State` type, `init` + `apply(&Transaction)` hooks
- [x] `PluginKey<P: Plugin>` zero-sized typed handle
- [x] `EditorState` holds a typed-erased map of plugin states (`EditorState::with_plugins`, `state.plugin(key)`)
- [~] `History` migration onto the trait — kept grandfathered for v0.2 via `HistoryIntent`; cosmetic migration tracked as v0.2.x

### Phase 3 — Markdown serializer / parser ✅

- [x] `taino_edit_core::markdown::to_markdown(doc)` (paragraphs, headings, blockquote, fenced code, bullet + ordered lists with start, strong/em/link marks, image)
- [x] `taino_edit_core::markdown::parse_markdown(schema, md)` via `pulldown-cmark` 0.13, schema-validated
- [x] DOM bridge: `EditorView::paste_markdown(md)`; Leptos adapter prefers `text/markdown` over `text/html` / `text/plain`

### Phase 4 — Dioxus adapter ✅ (minimum-viable)

- [x] `taino_edit_dioxus::TainoEditor` mounts + DOM-patches on signal change
- [x] `examples/basic-dioxus` (builds with `dx serve`)
- [~] Full event-wiring parity (input → transform, IME, paste, selectionchange) — deferred to v0.2.x

### Release ✅

- [x] Workspace version bumped to 0.2.0; docs refreshed; full gate sweep; `cargo publish` (6 crates); tag `v0.2.0`; GitHub Release.

---

## v0.2.x patch backlog

- [x] Full Dioxus event-wiring parity (input → transform round-trip, IME, paste, selectionchange) — done 2026-05-22; verified end-to-end in headless Chromium
- [x] Dioxus example toolbar + keymap parity (Bold/Italic/H1–H3/Undo/Redo) — done 2026-05-22
- [~] ~~Migrate `History` onto the `Plugin` trait~~ — **decided against** (2026-05-22). The `Plugin` trait is for *observer* plugins that fold state forward from transactions (`apply(tx, prev, state) -> state` cannot touch the doc). `History` is a *driver*: undo/redo rewrite the document via the `HistoryIntent` short-circuit and mutate their own stacks outside the normal apply path. Forcing it onto the trait would either bloat the trait with history-specific hooks or be a fake migration that's still special-cased. History stays a first-class `EditorState` field; the Plugin trait stays clean for observers.
- [x] Formal `wasm_bindgen_test` browser tests for the Dioxus adapter — done 2026-05-26; `tests/component.rs` mounts `<TainoEditor>` in a real `dioxus-web` tree in headless Chromium (paragraph + table-with-plugin)

---

## v0.3 — Shipped 2026-05-22

**Goal:** full tables, done at production quality, on a reusable
pointer-interaction platform.

- [x] `Table` extension: `table`/`table_row`/`table_cell`, colspan/rowspan/header/colwidth, HTML round-trip
- [x] Structural commands (insert, add/delete row & column, delete table, header toggle) — **span-correct** via a logical-grid placement model + compaction render
- [x] `Selection::Cell` core variant; `TableMap`; `select_cell_range`, `merge_cells`, `split_cell`
- [x] `set_column_width`; Tab/Shift-Tab cell navigation (chained keymap so it coexists with Lists)
- [x] `ViewPlugin` infrastructure in `taino-edit-dom` (DOM-aware event + decoration hooks, `pos_at_point`, nested-node decorations)
- [x] New crate `taino-edit-table-view` — `TableView`: cell drag-select, selection highlight, column-resize
- [x] Leptos `<TainoEditor>` `plugins` prop + pointer wiring; `basic-leptos` table toolbar + drag/resize
- [x] Interaction + invariant host tests + headless-Chromium browser tests (table rendering, ViewPlugin infra, TableView)

## Known issues

- 🐛 Applying a mark/block type to a multi-word selection occasionally leaves
  the trailing word(s) unformatted — an intermittent selection-boundary
  mapping issue (reported during v0.5 manual testing). Next-up to investigate.

## Deferred (v0.4+)

- [x] `schema!{}` DSL — sugar over the builder — done 2026-05-27. Implemented as a `macro_rules!` macro (not a proc-macro, per DESIGN_NOTES §6: no new crate, no deps), re-exported from the umbrella + adapters.
- 💤 `loro` integration behind `collab` feature — collaborative editing via Peritext CRDT
- 💤 Richer extensions: footnotes, mentions, math/KaTeX, embed
- [x] Wire `TableView` into the Dioxus adapter — done 2026-05-26; `ViewPlugins` prop + pointer wiring, `basic-dioxus` table toolbar, browser tests. Full event- and plugin-wiring parity with Leptos.
- [x] Inline (range-level) decorations for third-party UI (search highlight, comments) — done 2026-05-26; `Decoration::Inline`, drawn as an overlay layer (no text-node splitting, so the diff/patch read-back is untouched), contributed via `ViewPlugin::decorations`. Browser-tested incl. a read-back-safety test.
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
