# taino-edit — Design Notes

> Native Rust rich text editor (WYSIWYG) framework for Leptos and Dioxus.
> Inspired by ProseMirror / TipTap / Quill, but pure-Rust and reactive-first.

## 1. Why this project exists

State of the art (researched May 2026):

### Leptos
- **Papelito** (`msmaiaa/papelito`) — only attempt at a native WYSIWYG. **Archived October 2023**, 14 commits, no releases. Dead.
- **leptos-tiptap** (`lpotthast/leptos-tiptap`, v0.10.0 June 2025) — actively maintained but is a `wasm-bindgen` wrapper around the TypeScript TipTap bundle. Not pure Rust, requires JS at runtime.
- **tiptap-rs** — just WASM bindings to JS TipTap.
- **Leptonic** (the main component lib) ships an editor via leptos-tiptap → same JS dependency.

### Dioxus
- **dioxus-editor** (`exlee/dioxus-editor`) — 1 commit, "discovery phase", and it's a code editor, not WYSIWYG.
- **engrave**, "Fat Chance" — code editor experiments, not rich text.
- Dioxus exposes the `contenteditable` HTML attribute and a `TextEditable` trait in `dioxus-native-core`, but **no published WYSIWYG component crate exists**.

**Conclusion:** there is a real gap. No maintained, native, framework-reactive WYSIWYG editor exists for either framework. `taino-edit` aims to fill it.

## 2. Architecture

Layered, with a framework-agnostic core. Mirrors the ProseMirror+TipTap split.

```
taino-edit-core         framework-agnostic, no web-sys, no Leptos, no Dioxus
  document model        Node, Mark, Fragment, Schema
  transforms            Step, Transform, Mapping, Slice
  state                 EditorState, Selection, Transaction, Plugin
  history               undo/redo
  commands              toggleMark, setBlockType, lift, wrap, ...
  keymap + input rules
  serializers           HTML, JSON (Markdown optional)

taino-edit-dom          web bridge (web-sys, wasm-bindgen)
  ViewDesc / DOM diff
  Selection sync (window.getSelection ↔ core selection)
  MutationObserver → transactions
  IME composition, drag & drop, paste

taino-edit-leptos       thin adapter: Signals ↔ EditorState
taino-edit-dioxus       thin adapter: Signal ↔ EditorState
taino-edit-extensions   bold, italic, heading, list, link, image, code, blockquote
```

**Key insight:** ~80% of the code lives in `core` + `dom` and is reused between framework adapters. Each adapter should be ~500-1000 LOC.

**Future possibility (post-v1.0):** `taino-edit-blitz` or `taino-edit-freya` adapters would let the same editor run in **native GUI without DOM**. That's a differentiator no JS editor can match.

## 3. Scope decision

Total LOC reference points:
- ProseMirror core (6 packages): ~12-15k LOC
- TipTap framework + ~30 extensions: ~20k LOC
- Quill 2.x: ~15-20k LOC

Estimates for taino-edit:

| Component | MVP v0.1 | Production v1.0 |
|---|---|---|
| `core` | 5-7k | 10-12k |
| `dom` | 2-3k | 4-5k |
| `extensions` (12 basic) | 2.5-3k | 4-6k |
| `leptos` adapter | 0.5-1k | 1-1.5k |
| `dioxus` adapter | 0.5-1k | 1-1.5k |
| **Total source (no tests)** | **~11-15k LOC** | **~20-26k LOC** |

Honest time estimate: MVP v0.1 = 2-4 months full-time solo.

**Author context:** this is the second (and final for now) major contribution following `taino-dnd-*`. Scope must therefore be acutely realistic.

**v0.1 cut (broadened mid-Phase 7 — see ROADMAP for the dated decision):**

The original suggested cut was just `Bold / Italic / Heading / Paragraph / History`. While polishing for release, the author chose to **dream bigger** for v0.1 so the community has something they can drop into a real project without first writing four extensions themselves. The published v0.1 ships:

- `taino-edit-core` (model, transforms, state, history, commands, keymap, input rules)
- `taino-edit-dom` (`contenteditable` bridge: mount, diff/patch, selection sync, IME, paste, drag/drop, focus, decorations)
- `taino-edit-leptos` adapter (`<TainoEditor>`, browser-side event wiring incl. `selectionchange`)
- **12 extensions**:
  - inline marks: `Bold`, `Italic`, `Link`
  - block nodes: `Paragraph`, `Heading` (H1–H3), `Blockquote`, `CodeBlock`, `BulletList` / `OrderedList` / `ListItem` (Lists)
  - inline atoms: `Image`
  - attribute/selection commands: `Align` (text_align on paragraph/heading), `TransformCase` (upper/lower)
  - undo/redo: `History`

`taino-edit-dioxus` stays a placeholder; richer extensions (tables, footnotes, mentions, mathblock, …), nested-list indent/sink, smart Enter inside list items, generic plugin registry, inline-range decorations and the `loro`-backed `collab` feature are explicit v0.2 surface and remain community-contribution targets on top of the published core.

## 4. Naming

Chosen: **`taino-edit`** (continues the `taino-*` family established by `taino-dnd-*`).

Crates published to crates.io:
- `taino-edit` (umbrella re-export)
- `taino-edit-core`
- `taino-edit-dom`
- `taino-edit-leptos`
- `taino-edit-dioxus`
- `taino-edit-extensions`

Alternative considered (rejected, kept simple): `taino-areito` — Areíto being the Taíno word for ceremonial oral storytelling/narrative tradition. Author preferred the more direct `taino-edit`.

## 5. Reusable building blocks (don't reinvent)

- **Loro / crdt-richtext** — pure-Rust Peritext+Fugue CRDT engine. Could power optional collaborative mode without writing OT/CRDT from scratch.
- **`web-sys` Selection / Range APIs** — already complete, no FFI work needed for the DOM bridge.
- **rope data structures** — `ropey` or `xi-rope` for the text buffer if we go beyond a tree-of-nodes representation.

## 6. Resolved decisions (2026-05-15)

| # | Question | Decision | Why |
|---|---|---|---|
| 1 | First framework adapter | **Leptos** | Author's primary stack; Dioxus reserved for community / v0.2+ |
| 2 | Document model | **Tree-of-nodes (ProseMirror-style)** | Extensibility is the differentiator vs. existing JS wrappers; LOC budget already accounts for it |
| 3 | Schema definition | **Builder API for v0.1**, optional `schema!{}` macro in v0.2 | Avoids `proc-macro` crate overhead; macro is sugar over the builder, not architecture |
| 4 | License | **MIT OR Apache-2.0** dual | Rust ecosystem default; zero friction for downstream consumers |
| 5 | Repo layout | **Cargo workspace with `crates/` subdir** | Matches tokio/bevy/leptos convention; clean separation for 6 crates |
| 6 | CRDT (`loro`) integration | **Feature flag `collab`, off by default, not wired in v0.1** | `Step` will be designed to support `map_against(&Step)` so future opt-in doesn't force a `core` refactor |
| 7 | v0.1 extension breadth | **12 extensions instead of 5** (added 2026-05-21) | The earlier 5-extension cut was a polish-week minimum. A user dropping `taino-edit` into a real project needs lists, links, images and alignment to feel like a real editor — these are small, well-bounded additions (each is ~50–200 LOC) and the schema/command vocabulary already supports them. See `CHANGELOG.md` [0.1.0]. |

## 7. Locked technical choices

- **MSRV:** `1.80` — headroom for current Leptos features without chasing edge stable releases.
- **Leptos version:** pin to latest stable at scaffold time; isolate reactive API surface (`Signal`, `Effect`, `view!`) so future bumps are contained.
- **CI from commit 0:** GitHub Actions running `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --all-features`, `cargo doc --no-deps`.
- **Style:** rustfmt defaults; `#![deny(unsafe_code)]` everywhere except `taino-edit-dom` (which justifies any `unsafe` inline at the FFI boundary).
- **Versioning:** semver from v0.1.0; pre-1.0 minor bumps may break API. CHANGELOG kept Keep-a-Changelog style.
- **Implementation order and timeline:** see [ROADMAP.md](ROADMAP.md).

## 8. References

- ProseMirror: https://prosemirror.net/docs/
- TipTap: https://tiptap.dev/docs/editor/core-concepts/introduction
- Loro CRDT: https://www.loro.dev/blog/crdt-richtext
- Peritext paper: https://www.inkandswitch.com/peritext/
- awesome-leptos: https://github.com/leptos-rs/awesome-leptos
- leptos-tiptap (reference for what NOT to do, i.e. JS wrapper): https://github.com/lpotthast/leptos-tiptap
