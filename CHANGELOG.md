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
- Phase 2 — `taino-edit-core` transforms, state and history:
  - ProseMirror-ported `Node::replace` / `slice` / `cut` tree surgery.
  - `Step` trait + `ReplaceStep`, `ReplaceAroundStep`, `AddMarkStep`,
    `RemoveMarkStep`, `AttrStep` — each with `invert`, `map`, JSON;
    `step_from_json`. Designed for a future `map_against` (CRDT/OT).
  - `StepMap`/`Mapping` with deletion flags and mirror/recover.
  - `Transform` (step + mapping accumulator with editing helpers).
  - `Selection` (`Text`/`Node`/`All`), `EditorState`, `Transaction`,
    and a bounded, groupable undo/redo `History`.
- Phase 3 — `taino-edit-core` commands, keymap and input rules:
  - `Command` contract + `chain`; selection, mark, block and join
    commands; `Transform::split`.
  - Cross-platform `Keymap` (`Mod` = Ctrl/Cmd) and `base_keymap`
    (Enter/Backspace/Delete chains, `Mod-a`, caret motion).
  - Regex `InputRules`: `text_replace_rule`, `textblock_type_rule`
    (`## ` → heading), `wrapping_rule` (`> ` → blockquote).
  - `taino-edit-core` is now feature-complete for the v0.1 milestone.
- Phase 4 — `taino-edit-dom` contenteditable bridge:
  - `EditorView::mount` renders a `Node` into a real `contenteditable`,
    setting `tabindex="0"` for keyboard accessibility, and owns a
    `ViewDesc` tree mirroring the document.
  - Incremental `EditorView::update` patches the DOM in place:
    identical subtrees keep their nodes, text-only changes reuse the
    same `Text` node, only differing nodes are added/removed/replaced.
  - Bidirectional selection sync: `set_selection` writes the editor
    selection into `window.getSelection()`; `read_selection` translates
    the browser's anchor/focus back to a doc-level `Selection`.
  - `read_dom_changes()` produces a `Transform` from DOM-side text edits
    (typing or IME commits) — the algorithmic half of a
    `MutationObserver` adapter.
  - IME composition lifecycle (`composition_start`/`composition_end`/
    `is_composing`): transient glyph states never produce transactions.
  - Clipboard paste: `paste_text` (plain) and `paste_html` (sanitized
    through the schema's strict, depth-bounded `parse_html`).
  - Drag-and-drop primitives: `extract_slice` and `drop_slice`.
  - Focus management: `focus`/`has_focus`/`set_tabindex`.
  - Node decorations: a CSS class on a block element.
  - `vendor/wasm-bindgen-cli-w3c-0.2.121.patch` plus
    `scripts/install-wasm-test-runner.sh` and `scripts/wasm-test.sh`
    make the wasm-bindgen browser-test pipeline reproducible: 46
    `wasm_bindgen_test` cases pass in headless Chromium 148.

- Phase 5 — `taino-edit-leptos` adapter:
  - `<TainoEditor>` component takes a single `RwSignal<EditorState>`;
    mount/diff happen automatically through a Leptos `Effect`, with the
    `EditorView` parked in a `StoredValue<…, LocalStorage>` so the
    `!Send` view can live next to Send+Sync effect closures.
  - Browser events wired: `input` -> `read_dom_changes` -> commit,
    `compositionstart`/`compositionend` -> IME lifecycle, `paste` ->
    `paste_html`/`paste_text` (sanitized). Every listener is dropped on
    `on_cleanup` via an RAII `EventCloser`.
  - `taino_edit_core::Step` gains `Send + Sync` bounds so
    `RwSignal<EditorState>` can live in Leptos's default `SyncStorage`.
  - Curated re-exports: `SchemaBuilder`, `NodeSpec`, `MarkSpec`,
    `EditorState`, `Selection`, `Transaction`, the standard commands,
    `base_keymap`, `EditorView`, `Decoration`, …
  - `examples/basic-leptos/` is a `trunk serve`-buildable demo with
    Bold/Undo/Redo buttons + a mounted editor.
  - 6 wasm_bindgen_test cases drive the component through Leptos's CSR
    runtime in headless Chromium 148.

- Phase 6 — `taino-edit-extensions`:
  - `Extension` trait + `SchemaAdditions` + helpers `build_schema_with`,
    `build_keymap_with` so adapter consumers can compose extensions on
    top of a user-supplied base schema builder and the platform's
    `base_keymap`.
  - Built-in extensions: `Bold` (`strong`, `Mod-b`), `Italic` (`em`,
    `Mod-i`), `Heading` (`level` attr, `Mod-Alt-1..3`), `Paragraph`
    (`Mod-Alt-0`), `History` (`Mod-z` / `Mod-Shift-z`).
  - Core gains `HistoryIntent` + `Transaction::set_history_intent`; the
    `History` extension tags its undo/redo transactions so
    `EditorState::apply` walks the undo/redo stack instead of pushing
    a new history entry — the standard `Command` / `Transaction`
    pipeline now handles undo/redo without a special path.
  - `Keymap::add` exposed so extensions can splice bindings on top of
    `base_keymap` without rebuilding it.

[Unreleased]: https://github.com/juanma-dev/taino-edit/commits/main
