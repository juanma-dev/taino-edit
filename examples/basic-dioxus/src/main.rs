//! taino-edit + Dioxus demo: a `<TainoEditor>` with a toolbar, the
//! `Mod-…` keymap wired on keydown, and live HTML + JSON panels. Build
//! with `dx serve` (or `trunk serve`) in this directory.
//!
//! The Dioxus adapter has full event- and plugin-wiring parity with the
//! Leptos one (input → transform, IME composition, paste, selectionchange,
//! plus `ViewPlugin` pointer events) — type into the editor, click the
//! toolbar, drag across table cells, and watch the panels track the
//! document state.

use dioxus::prelude::*;
use taino_edit_dioxus::{
    set_block_type, toggle_mark, Attrs, Command, EditorState, KeymapProp, NodeSpec, SchemaBuilder,
    TainoEditor, Transaction, ViewPlugins,
};
use taino_edit_extensions::{
    add_column_after, add_row_after, build_keymap_with, build_schema_with, delete_table,
    insert_table, merge_cells, redo_command, select_cell_range, split_cell, toggle_header_row,
    undo_command, Bold, Heading, History, Italic, Paragraph, Table,
};
use taino_edit_table_view::TableView;

fn main() {
    dioxus::launch(App);
}

/// Apply a command against the editor state signal.
fn run_cmd(mut state: Signal<EditorState>, cmd: Command) {
    let snap = state.peek().clone();
    let mut next = None;
    {
        let mut d = |tx: Transaction| next = Some(snap.apply(tx));
        cmd(&snap, Some(&mut d));
    }
    if let Some(n) = next {
        state.set(n);
    }
}

/// Build a `set_block_type("heading", level=n)` command.
fn heading_cmd(level: u64) -> Command {
    set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(level))]),
    )
}

#[component]
fn App() -> Element {
    let base = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        );
    let exts: Vec<&dyn taino_edit_extensions::Extension> =
        vec![&Paragraph, &Heading, &Bold, &Italic, &History, &Table];
    let schema = build_schema_with(base, &exts, "doc").expect("schema builds");

    let title = schema
        .text("Welcome to taino-edit (Dioxus)", vec![])
        .unwrap();
    let h = schema
        .node(
            "heading",
            Attrs::from_iter([("level".into(), serde_json::json!(1u64))]),
            vec![title],
            vec![],
        )
        .unwrap();
    let body = schema
        .text(
            "Type, format, undo — every command goes through Rust.",
            vec![],
        )
        .unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![body], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![h, p], vec![])
        .unwrap();

    // Build the keymap once from the same extension set. `<TainoEditor>`
    // owns `keydown` (synchronous, live-selection), so we just hand it the
    // keymap via the `KeymapProp` wrapper.
    let keymap = {
        let exts: Vec<&dyn taino_edit_extensions::Extension> =
            vec![&Paragraph, &Heading, &Bold, &Italic, &History, &Table];
        build_keymap_with(&exts, &schema, /*mac=*/ false)
    };
    let state = use_signal(|| EditorState::new(doc, schema));

    // Toolbar buttons run a command and keep editor focus (mousedown
    // preventDefault stops the button from stealing the selection).
    let mark_cmd = move |mark: &str| {
        let schema = state.peek().schema().clone();
        if let Some(mt) = schema.mark_type(mark) {
            run_cmd(state, toggle_mark(mt.clone()));
        }
    };

    // Merge the caret's whole row: select it end-to-end, then merge. The
    // demo inserts 3×3 tables, so columns 0..=2 of row 0 cover a full row;
    // a real app would drive `select_cell_range` from a mouse drag.
    let on_merge_row = move |_| {
        run_cmd(state, select_cell_range((0, 0), (0, 2)));
        run_cmd(state, merge_cells());
    };

    rsx! {
        main {
            style: "font-family: system-ui; max-width: 60rem; margin: 1.5rem auto; padding: 0 1rem;",
            h1 { "taino-edit + Dioxus demo" }
            p {
                style: "color:#555;",
                "Pure-Rust WYSIWYG running in Dioxus. Use the toolbar or the "
                "Mod-… shortcuts; the live HTML and JSON panels track every edit."
            }

            div {
                role: "toolbar",
                style: "display:flex; flex-wrap:wrap; gap:.4rem; margin-bottom:.5rem;",
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| mark_cmd("strong"),
                    "Bold (Mod-b)"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| mark_cmd("em"),
                    "Italic (Mod-i)"
                }
                span { style: "width:.5rem" }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, set_block_type("paragraph", Attrs::new())),
                    "Paragraph (Mod-Alt-0)"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, heading_cmd(1)),
                    "H1 (Mod-Alt-1)"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, heading_cmd(2)),
                    "H2 (Mod-Alt-2)"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, heading_cmd(3)),
                    "H3 (Mod-Alt-3)"
                }
                span { style: "width:.5rem" }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, undo_command()),
                    "Undo (Mod-z)"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, redo_command()),
                    "Redo (Mod-Shift-z)"
                }
                span { style: "width:.5rem" }
                strong { style: "font-size:.8rem; color:#555; align-self:center;", "Table:" }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, insert_table(3, 3)),
                    "⊞ Insert 3×3"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, add_row_after()),
                    "+ Row"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, add_column_after()),
                    "+ Col"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, toggle_header_row()),
                    "Header row"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: on_merge_row,
                    "Merge row"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, split_cell()),
                    "Split cell"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, delete_table()),
                    "Delete table"
                }
            }

            TainoEditor {
                state,
                keymap: KeymapProp::new(keymap),
                plugins: ViewPlugins::new(vec![Box::new(TableView::new())]),
            }

            section {
                style: "margin-top:1.5rem; display:grid; grid-template-columns:1fr 1fr; gap:1rem;",
                div {
                    h2 { style: "font-size:1rem;", "Live HTML" }
                    pre {
                        style: "background:#f6f8fa; padding:.75rem; border-radius:4px; overflow:auto; max-height:18rem; font-size:.85rem;",
                        "{state.read().doc().to_html()}"
                    }
                }
                div {
                    h2 { style: "font-size:1rem;", "Live JSON" }
                    pre {
                        style: "background:#f6f8fa; padding:.75rem; border-radius:4px; overflow:auto; max-height:18rem; font-size:.85rem;",
                        "{serde_json::to_string_pretty(&state.read().doc().to_json()).unwrap_or_default()}"
                    }
                }
            }
        }
    }
}
