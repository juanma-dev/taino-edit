# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Pre-1.0, minor version bumps may include breaking API changes.

## [Unreleased]

### Added

- **`schema! { .. }` macro** — declarative sugar over `SchemaBuilder` (a
  `macro_rules!` macro, no proc-macro crate per DESIGN_NOTES §6). Declare nodes
  and marks as compact `name { key: value, .. }` blocks instead of full
  `NodeSpec { .. ..Default::default() }` literals: `content` / `group` /
  `marks` strings, `inline` / `atom` bools, a `dom: "tag"` render shorthand or
  explicit `to_dom`, a `parse: ["tag", ..]` list, and an `attrs: { name:
  default }` block. Yields `Result<Schema, _>`. Re-exported from the umbrella
  crate and the Leptos/Dioxus/extensions facades.
- **Inline (range-level) decorations.** A new `Decoration::Inline { from, to,
  class }` variant highlights an arbitrary inline range — search hits, comment
  ranges, collaborative remote selections — for third-party UI. It is drawn as
  an **overlay** (absolutely-positioned boxes, one per client rect, layered
  above the text in a sibling-of-root layer) rather than by wrapping text in a
  `<span>`. Wrapping would split the editable text nodes that the diff/patch
  read-back relies on; the overlay leaves the editable DOM untouched, so typing
  and reconciliation are unaffected. Plugins contribute them through the
  existing `ViewPlugin::decorations` hook. Constructors: `Decoration::node(..)`
  / `Decoration::inline(..)`.

## [0.4.0] - 2026-05-26

The **Dioxus adapter reaches full `ViewPlugin` parity** with Leptos: tables
are now interactive (cell drag-select, selection highlight, column resize) in
both frameworks, on the same framework-agnostic plugin platform — verified by
a new headless-Chromium browser suite for the Dioxus adapter.

### Added

- **Dioxus adapter: `ViewPlugin` parity with Leptos.** `TainoEditor` (Dioxus)
  gained a `plugins` prop — a new `ViewPlugins` container — so DOM-aware
  plugins like `TableView` (cell drag-select, selection highlight, column
  resize) now work in Dioxus too. The component wires `mousedown`/`mousemove`/
  `mouseup` to the plugins and refreshes their decorations on every state
  change, giving full event- **and plugin**-wiring parity with
  `taino-edit-leptos`. The `basic-dioxus` example gains a table toolbar +
  `TableView`.
- **Formal browser tests for the Dioxus adapter.** `tests/component.rs` mounts
  `<TainoEditor>` in a real `dioxus-web` render tree in headless Chromium,
  covering initial-document mount, `contenteditable`, and a table rendered
  through the `TableView` plugin — closing the last deferred Dioxus-parity
  item.

## [0.3.1] - 2026-05-25

### Added

- **`Code` inline-mark extension** — a `code` mark (`<code>`, toggled with
  `Mod-e`) for inline code spans, distinct from the `CodeBlock` `<pre>`
  block. Round-trips to Markdown backticks (`` `like this` ``); the
  Markdown parser now applies the mark to inline code (previously dropped
  to plain text), and the serializer emits literal, un-escaped backtick
  spans (auto-widening the fence when the content contains backticks).
  Wired into the `basic-leptos` toolbar.

## [0.3.0] - 2026-05-22

Full tables — schema, span-correct structural editing, cell selection +
merge/split, and pointer interaction (cell drag-select, column resize) —
on a new reusable `ViewPlugin` platform. Plus the Dioxus adapter reaches
event-wiring parity with Leptos.

### Highlights

- A complete **`Table`** extension (`table`/`table_row`/`table_cell`,
  colspan/rowspan/header/colwidth) whose every command is **span-correct**
  — interleaving merge with add/delete row/column can never produce an
  orphan span or an empty row (enforced by a logical-grid placement model
  + compaction render, covered by interaction + invariant tests).
- A new **`ViewPlugin`** infrastructure in `taino-edit-dom` (DOM-aware
  event + decoration hooks, `pos_at_point`, nested-node decorations) and a
  new crate **`taino-edit-table-view`** implementing table cell
  drag-select, selection highlight and column-resize on top of it.
- The **Dioxus adapter** now has full event-wiring parity with Leptos.
- 184 host tests + the wasm-bindgen browser suite (incl. table rendering,
  the ViewPlugin infra, and TableView pointer interaction) all pass in
  headless Chromium 148.

### Added

- **`Table` extension** — a full table feature set:
  - **Nodes**: `table` / `table_row` / `table_cell` with `<table><tr>
    <td>`/`<th>` HTML round-trip. Cells carry `colspan` / `rowspan` /
    `header` / `colwidth` attrs.
  - **Structural commands**: `insert_table(rows, cols)`, `add_row_before`
    / `add_row_after` / `delete_row`, `add_column_before` /
    `add_column_after` / `delete_column`, `delete_table`. Deleting the
    last row or column removes the whole table.
  - **Header toggling**: `toggle_header_row` / `toggle_header_column` /
    `toggle_header_cell` (flip cells between `<td>` and `<th>`).
  - **Cell navigation**: `go_to_next_cell` (`Tab`) / `go_to_prev_cell`
    (`Shift-Tab`); next-past-the-last-cell appends a row. Coexists with
    the Lists `Tab` binding via the new chained-keymap composition.
  - **Cell-range selection + merge/split**: `Selection::Cell` (new core
    variant), a `TableMap` resolving the logical grid through
    colspan/rowspan, `select_cell_range`, `merge_cells` (rectangle →
    spanned cell with concatenated content) and `split_cell` (spanned
    cell → 1×1 cells).
  - **Column resize**: `colwidth` attr serialized as `style="width:
    Npx"`, set via `set_column_width(col, width)`.
  - **Span-correct everywhere**: every structural command (and merge/
    split) goes through a logical-grid placement model + compaction
    render, so they can never leave an orphan `rowspan`/`colspan` or an
    empty `<tr>` — even when interleaved (merge then add-column, delete
    through a span, …). Verified by interaction + invariant tests.
  - The `basic-leptos` demo gains a table toolbar (insert / row / column
    / header / merge / split / resize / delete).
- **`ViewPlugin` infrastructure** (`taino-edit-dom`): a DOM-aware plugin
  trait (`handle_event` → `ViewAction::Select`/`Command`, `decorations`)
  that `EditorView` consults, plus the `pos_at_point` / `node_dom_at`
  primitives and recursive (nested-node) decoration support. Adapters
  wire pointer events to `EditorView::handle_view_event` and refresh
  plugin decorations via `refresh_view_decorations`. The Leptos
  `<TainoEditor>` gains an optional `plugins` prop and the pointer
  wiring.
- **`taino-edit-table-view` crate** — `TableView`, a `ViewPlugin` giving
  tables their pointer interaction: **cell drag-select** (build a
  `Selection::Cell` by dragging), **selection highlight** (covered cells
  get `taino-cell-selected` via decorations), and **column resize**
  (drag near a cell's right border → `set_column_width`). Verified
  end-to-end in headless Chromium, including in the live `basic-leptos`
  demo.
- **`Selection::Cell { anchor, head }`** in `taino-edit-core` — a
  positional table cell-range selection (table-aware code interprets the
  rectangle; `core` handles `from`/`to`/`map` generically).
- **`Keymap::add_chained`** + chain-on-conflict in `build_keymap_with`:
  when two extensions bind the same key, the bindings chain (later tried
  first, earlier as fallback) instead of overriding — so `Tab` runs
  cell-navigation in a table and list-indent in a list.

### Docs

- Recorded the decision to keep `History` as a first-class `EditorState`
  field rather than migrating it onto the `Plugin` trait: the trait is
  for *observer* plugins (fold state forward from transactions, can't
  touch the doc), while `History` is a *driver* (undo/redo rewrite the
  doc). See the `plugin` module docs and `ROADMAP.md`.

### Changed

- **Dioxus adapter — full event-wiring parity.** `taino-edit-dioxus`
  now wires `input` (→ transform round-trip), IME `compositionstart` /
  `compositionend`, `paste` (Markdown / HTML / plain-text, sanitized
  through core), and document `selectionchange` — the same raw `web-sys`
  listeners the Leptos adapter uses, kept alive in the component's
  runtime slot. Typing in a Dioxus-hosted editor now commits to the
  state signal (verified end-to-end in headless Chromium). This closes
  the v0.2.0 "minimum-viable adapter" gap.
- `examples/basic-dioxus` gains a full toolbar (Bold / Italic /
  Paragraph / H1–H3 / Undo / Redo), the `Mod-…` keymap wired on
  keydown, and live HTML + JSON panels — matching the Leptos demo. Also
  a `<div id="main">` mount target + trunk `index.html` so it serves
  under both `dx serve` and `trunk serve`.

## [0.2.0] - 2026-05-21

Closes the v0.1 list UX gaps and broadens the platform: a second
framework adapter, a stateful-plugin trait, and Markdown round-trip.

### Highlights

- **List UX completion.** Smart Enter inside a list item (splits both
  the textblock and the enclosing list_item), sink/dedent (`Tab` →
  nest under previous sibling), and multi-item `lift_list_item` that
  preserves the surviving siblings. Empty-bullet + Enter exits the
  list.
- **`Plugin` trait + `PluginKey` + typed-state registry** in `core`.
  Third-party stateful components (word counters, autosave, future
  CRDT bridges) plug into `EditorState` without forking core. The
  built-in `History` machinery stays grandfathered for back-compat.
- **Markdown round-trip.** New `taino_edit_core::markdown` module:
  `to_markdown(node)` serializes to a CommonMark subset; `parse_markdown`
  tokenises via `pulldown-cmark` and validates the result against the
  schema. `EditorView::paste_markdown` is wired; the Leptos adapter
  now prefers `text/markdown` over `text/html` / `text/plain` when the
  clipboard advertises it.
- **`taino-edit-dioxus` adapter** ships as a real, minimum-viable
  adapter (previously a name-reservation placeholder). Mount + DOM
  patching prove the architecture is framework-agnostic in practice.
  `examples/basic-dioxus` builds with `dx serve`.
- `Transform::split_at_depth(pos, levels, schema)` — generalised the
  single-depth `split` so callers can do multi-level structural splits
  (smart Enter uses it for paragraph + list_item).

### Added

- **List commands**: `split_list_item` / `smart_enter_in_list` /
  `sink_list_item`; `lift_list_item` generalised. `Lists` keymap now
  binds `Tab` and `Enter` (chained with `split_block` so the binding
  only fires inside a list).
- **Plugin platform**: `Plugin` trait, `PluginKey<P>`, `PluginSet`,
  `EditorState::with_plugins(...)`, `EditorState::plugin(key)`.
- **Markdown**: `taino_edit_core::markdown::{to_markdown,
  parse_markdown}`, `EditorView::paste_markdown(md)`, and adapter
  paste-prefers-markdown wiring.
- **Dioxus**: `taino_edit_dioxus::TainoEditor` component, full
  curated re-exports of the core types. The umbrella `taino-edit`
  crate's `dioxus` feature now wires through to functionality instead
  of an empty crate.
- **Transform helper**: `Transform::split_at_depth(pos, levels, schema)`.

### Changed

- `taino-edit-core` now depends on `pulldown-cmark` (default-features
  off) for the Markdown parser.
- `EditorState` carries a typed-erased plugin-state registry alongside
  its existing `History`. `EditorState::new(doc, schema)` keeps
  working with an empty plugin set, so v0.1 callers are unaffected.

### Known limitations / deferred

- Migrating `History` onto the `Plugin` trait (it works via a
  dedicated `HistoryIntent` short-circuit; the migration is cosmetic
  and tracked as a v0.2.x patch task).
- Full event-wiring parity for the Dioxus adapter (input → transform
  round-trip, IME, paste, selectionchange) — the dom-layer pieces are
  shipped; wiring lands in v0.2.x.
- Generic plugin lifecycle hooks beyond `init` + `apply` (e.g.
  `view_props`, `destroy`) — out of v0.2 scope.

## [0.1.0] - 2026-05-21

The first publishable release of taino-edit. A pure-Rust ProseMirror-style
WYSIWYG editor with a Leptos adapter, no JavaScript dependency at runtime.

### Highlights

- Framework-agnostic typed document model + invertible transforms +
  bounded undo/redo, all in pure-Rust `taino-edit-core`.
- Schema-checked JSON and HTML round-trip; the HTML parser is strict,
  dependency-free and depth-bounded — untrusted clipboard content
  cannot inject markup.
- A real `contenteditable` DOM bridge with diff/patch, bidirectional
  selection sync, IME composition, sanitized paste, drag/drop primitives,
  focus management and node decorations.
- An idiomatic Leptos component (`<TainoEditor state=signal />`) backed
  by a `RwSignal<EditorState>`, with browser events wired through
  automatically.
- **Twelve** built-in extensions, enough to drop into a real project:
  inline marks (`Bold`, `Italic`, `Link`), block nodes (`Paragraph`,
  `Heading` H1–H3, `Blockquote`, `CodeBlock`, the `Lists` trio
  `BulletList`/`OrderedList`/`ListItem`), inline atoms (`Image`),
  attribute and selection commands (`Align`, `TransformCase`), and
  `History` (`Mod-z` / `Mod-Shift-z`).
- 110 host tests + 52 `wasm_bindgen_test` cases in headless Chromium 148.

### Known limitations / explicitly deferred to v0.2

- Smart **Enter** inside a list item (`split_list_item`) and sink/dedent
  for **nested lists** — the wrap and lift primitives are there; the
  multi-level slice surgery is a v0.2 follow-up.
- Multi-item `lift_list_item` (v0.1 covers the single-item case
  cleanly).
- Generic `Plugin` trait + `PluginKey` typed-state registry — v0.1 ships
  `History` as the only built-in stateful component.
- Inline (range-level) decorations — node decorations only in v0.1.
- A richer per-node `NodeView` trait with imperative DOM hooks.
- The Dioxus adapter (placeholder crate is reserved).
- `loro` CRDT integration behind the `collab` feature flag.
- Markdown serializer/parser.
- Richer extensions on top of the v0.1 twelve: tables, footnotes,
  mentions, math/KaTeX, embed.
- Counted-range content quantifiers `{n,m}`.
- Full WCAG accessibility audit (tabindex/focus + contenteditable
  defaults are wired; deeper a11y review is post-1.0).

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

- Phase 7 — polish for the v0.1.0 release:
  - `examples/headless-core/` — a CLI/server-side demo that proves
    `taino-edit-core` runs identically without any DOM (compose schema,
    edit through `Transform`, JSON + HTML round-trip, command + undo).
  - README rewritten to reflect the actual feature set, test counts and
    explicitly-deferred items.

- Phase 6 — `taino-edit-extensions`:
  - `Extension` trait + `SchemaAdditions` + helpers `build_schema_with`,
    `build_keymap_with` so adapter consumers can compose extensions on
    top of a user-supplied base schema builder and the platform's
    `base_keymap`.
  - Initial cut (five built-ins): `Bold` (`strong`, `Mod-b`), `Italic`
    (`em`, `Mod-i`), `Heading` (`level` attr, `Mod-Alt-1..3`),
    `Paragraph` (`Mod-Alt-0`), `History` (`Mod-z` / `Mod-Shift-z`).
  - Core gains `HistoryIntent` + `Transaction::set_history_intent`; the
    `History` extension tags its undo/redo transactions so
    `EditorState::apply` walks the undo/redo stack instead of pushing
    a new history entry — the standard `Command` / `Transaction`
    pipeline now handles undo/redo without a special path.
  - `Keymap::add` exposed so extensions can splice bindings on top of
    `base_keymap` without rebuilding it.
  - **v0.1 broadening (2026-05-21)** — seven additional extensions, all
    built on the existing schema/command/keymap surface (no `core`
    architectural changes):
    - `Link` — `<a href title>` mark with `set_link(href, title?)` and
      `remove_link` commands (no default binding; the host wires a
      URL prompt).
    - `Image` — inline `<img>` atom with `src`/`alt`/`title` attrs and
      an `insert_image(src, alt?)` command.
    - `Align` — `text_align` attribute on `paragraph` and `heading`
      with four commands and bindings `Mod-Shift-{l,e,r,j}`; serializes
      as `style="text-align: …"`.
    - `TransformCase` — selection-scoped `to_uppercase` /
      `to_lowercase` commands.
    - `Blockquote` — `<blockquote>` wrapper bound to `Mod->`.
    - `CodeBlock` — `<pre>` block bound to `` Mod-` ``.
    - `Lists` — `bullet_list` / `ordered_list` / `list_item` nodes,
      `wrap_in_bullet_list` (`Mod-Shift-8`), `wrap_in_ordered_list`
      (`Mod-Shift-7`) and `lift_list_item` (`Shift-Tab`).
  - Keymap improvement to make symbol-key bindings work as users expect:
    `Keymap::handle` now does a two-pass lookup — exact canonical first,
    then with Shift stripped when the press key isn't a lowercase ASCII
    letter — so `Mod->` matches a Ctrl+Shift+> press without spelling
    out `Shift` in the binding.

- Phase 7 — UX bug fix:
  - `<TainoEditor>` now registers a `document.selectionchange` listener
    and folds the browser selection back into `state.selection`. Before
    this, toolbar buttons that depend on the caret (set_block_type for
    H1/H2/H3, the new `align_*` and `set_link` commands, …) acted on
    whatever block was selected at mount time instead of where the user
    actually clicked. The effect also re-pushes `state.selection` onto
    the DOM after every state-driven update, so commands that move the
    caret are reflected visually. Both directions guard against echo
    loops via an `applying_selection` flag.

[Unreleased]: https://github.com/juanma-dev/taino-edit/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/juanma-dev/taino-edit/releases/tag/v0.4.0
[0.3.1]: https://github.com/juanma-dev/taino-edit/releases/tag/v0.3.1
[0.3.0]: https://github.com/juanma-dev/taino-edit/releases/tag/v0.3.0
[0.2.0]: https://github.com/juanma-dev/taino-edit/releases/tag/v0.2.0
[0.1.0]: https://github.com/juanma-dev/taino-edit/releases/tag/v0.1.0
