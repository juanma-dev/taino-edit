# taino-edit

> Native Rust WYSIWYG rich-text editor framework for [Leptos](https://leptos.dev) — pure Rust at runtime, **no JavaScript bridge**.

[![CI](https://github.com/juanma-dev/taino-edit/actions/workflows/ci.yml/badge.svg)](https://github.com/juanma-dev/taino-edit/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

`taino-edit` is a ProseMirror/TipTap-inspired editor — typed document model,
invertible transforms, history, commands and a Leptos component — built
reactive-first for Rust web frameworks. Unlike `leptos-tiptap` (a
`wasm-bindgen` wrapper around the TypeScript TipTap bundle), there is **no
JS dependency at runtime**.

It is part of the `taino-*` family, following `taino-dnd-*`.

## Status: v0.2.0 released

Six crates on crates.io. v0.2 closes the v0.1 list UX gaps and broadens
the platform (Plugin trait, Markdown round-trip, Dioxus adapter ships
real functionality). Tests pass workspace-wide:

| | |
|---|---|
| Host tests | **145** (model, schema, content automaton, replace, steps, transforms, state, history, commands, keymap, input-rules, plugin registry, Markdown serializer + parser, and **12 extensions** including the full list UX) |
| Browser tests | **52** `wasm_bindgen_test` cases in headless Chromium 148 (mount, diff/patch, selection sync, DOM-typing → Transform, IME, clipboard, drag/drop, focus, decorations, Leptos component + event wiring) |

See **[DESIGN_NOTES.md](DESIGN_NOTES.md)** for the architecture, the
scope budget, and the resolved design decisions; **[ROADMAP.md](ROADMAP.md)**
tracks phase progress and what's deferred to v0.2.

## What's new in v0.2 (2026-05-21)

- **Complete list UX**: smart Enter inside a list item splits the
  list_item (new bullet below), empty bullet + Enter exits the list,
  `Tab` nests under the previous sibling, `Shift-Tab` lifts out, and
  multi-item lift now preserves surviving siblings.
- **`Plugin` trait + `PluginKey` + typed-state registry** in `core`:
  third-party stateful components (word counters, autosave, future
  CRDT bridges) plug into `EditorState` without forking core.
- **Markdown round-trip**: `taino_edit_core::markdown::{to_markdown,
  parse_markdown}` + `EditorView::paste_markdown`; the Leptos adapter
  prefers `text/markdown` on paste when the clipboard advertises it.
- **`taino-edit-dioxus`** ships as a real, minimum-viable adapter
  (mount + DOM patch on signal change). [`examples/basic-dioxus`](examples/basic-dioxus/)
  builds with `dx serve`.

## What ships in v0.1

- A typed, immutable document tree (ProseMirror-style `Node` /
  `Mark` / `Fragment` / `Slice`).
- A `Schema` + `SchemaBuilder` with a Thompson-NFA-to-DFA content automaton
  (`paragraph+`, `(text | image)*`, `+ * ?`).
- Schema-checked **JSON** round-trip (`Node::to_json` ↔ `Schema::node_from_json`).
- A dependency-free escaped **HTML** serializer and a strict, depth-bounded
  HTML parser (rejects unknown tags, can't be tricked into injecting markup).
- Invertible, mappable **Step**s (`ReplaceStep`, `ReplaceAroundStep`,
  `AddMark`/`RemoveMark`/`AttrStep`), `Mapping` with mirror/recover, and a
  `Transform` builder.
- An `EditorState` with `Selection`, `Transaction`, and a bounded
  groupable **undo/redo** `History`.
- A standard command vocabulary (`select_all`, `toggle_mark`,
  `set_block_type`, `wrap_in`, `lift`, `split_block`, `join_…`, …), a
  cross-platform `Keymap` (`Mod` = Ctrl/Cmd) and a `base_keymap`.
- Regex **input rules** (`## ` → heading, `> ` → blockquote, …).
- A real **`contenteditable` DOM bridge** (`taino-edit-dom`): mount,
  incremental diff/patch, bidirectional selection sync, IME composition,
  clipboard paste sanitized through the schema, drag-and-drop primitives,
  focus management and node decorations.
- A first-class **Leptos** adapter: `<TainoEditor state=signal />` mounts
  the editor, wires every event (including `selectionchange`) back
  through the state signal, and is tested inside the real Leptos CSR
  runtime.
- **Twelve built-in extensions**, enough to drop into a real project:
  - *Inline marks:* `Bold` (`Mod-b`), `Italic` (`Mod-i`), `Link`
    (`set_link` / `remove_link` commands; the host wires the URL
    prompt).
  - *Block nodes:* `Paragraph` (`Mod-Alt-0`), `Heading` H1–H3
    (`Mod-Alt-1..3`), `Blockquote` (`Mod->`), `CodeBlock`
    (`` Mod-` ``), and the `Lists` trio (`BulletList`/`OrderedList`
    + `ListItem`, `Mod-Shift-8`/`Mod-Shift-7` + `Shift-Tab` to lift).
  - *Inline atoms:* `Image` (`insert_image` command).
  - *Attribute / selection commands:* `Align`
    (`align_left/center/right/justify`, `Mod-Shift-{l,e,r,j}`),
    `TransformCase` (`to_uppercase` / `to_lowercase`).
  - *Undo/redo:* `History` (`Mod-z` / `Mod-Shift-z`).

Explicitly deferred to v0.2: generic plugin registry, inline-range
decorations, a richer per-node `NodeView` trait, the Dioxus adapter,
`loro` CRDT integration behind a `collab` feature, Markdown
serializer/parser, smart Enter / nested-list sink (indent) for lists,
and richer extensions (tables, footnotes, mentions, math).

## Workspace layout

| Crate                                                  | Role                                                              |
| ------------------------------------------------------ | ----------------------------------------------------------------- |
| [`taino-edit-core`](crates/taino-edit-core)             | Framework-agnostic model, transforms, state, history, commands, keymap, input rules |
| [`taino-edit-dom`](crates/taino-edit-dom)               | `contenteditable`/DOM bridge (`web-sys`, `wasm-bindgen`, `js-sys`) |
| [`taino-edit-extensions`](crates/taino-edit-extensions) | `Bold`/`Italic`/`Heading`/`Paragraph`/`History`, plus the `Extension` trait |
| [`taino-edit-leptos`](crates/taino-edit-leptos)         | Leptos adapter (`<TainoEditor>`); first-class for v0.1            |
| [`taino-edit-dioxus`](crates/taino-edit-dioxus)         | Placeholder, reserved for v0.2                                    |
| [`taino-edit`](crates/taino-edit)                       | Umbrella crate, feature-gated re-exports                           |

Examples under [`examples/`](examples/):

- [`basic-leptos`](examples/basic-leptos) — a `trunk serve`-buildable demo
  with Bold/Undo/Redo buttons and a mounted editor.
- [`headless-core`](examples/headless-core) — server-side / CLI demo
  proving `taino-edit-core` runs identically without a DOM.

## Install

```toml
[dependencies]
taino-edit = { version = "0.2", features = ["leptos"] }  # or "dioxus"
```

No adapter is enabled by default — pick `leptos` or `dioxus`.

## Use it (Leptos)

```rust,no_run
use leptos::prelude::*;
use taino_edit_leptos::{
    build_keymap_with, build_schema_with, Bold, DomSpec, EditorState,
    Italic, NodeSpec, SchemaBuilder, TainoEditor,
};

#[component]
fn App() -> impl IntoView {
    // Compose a schema on top of the universal doc/text primitives.
    let base = SchemaBuilder::new()
        .node("doc",  NodeSpec { content: Some("block+".into()),  ..Default::default() })
        .node("text", NodeSpec { group:   Some("inline".into()),  ..Default::default() });
    // Paragraph etc. come from `taino-edit-extensions`.
    let exts: Vec<&dyn taino_edit_extensions::Extension> =
        vec![&taino_edit_extensions::Paragraph, &Bold, &Italic];
    let schema = build_schema_with(base, &exts, "doc").unwrap();

    let txt   = schema.text("Hello from Rust!", vec![]).unwrap();
    let para  = schema.node("paragraph", Default::default(), vec![txt], vec![]).unwrap();
    let doc   = schema.node("doc",       Default::default(), vec![para], vec![]).unwrap();
    let state = RwSignal::new(EditorState::new(doc, schema));

    view! { <TainoEditor state=state /> }
}
```

## Build & test

Requires the Rust toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml)
(stable, MSRV 1.80).

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

Browser tests for `taino-edit-dom` and `taino-edit-leptos` use a small
locally-patched `wasm-bindgen-cli`; first time only run
`./scripts/install-wasm-test-runner.sh`, after that
`./scripts/wasm-test.sh` runs them in headless Chromium 148. See
[`vendor/README.md`](vendor/README.md) for the rationale.

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
