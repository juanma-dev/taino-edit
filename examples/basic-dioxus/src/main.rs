//! taino-edit + Dioxus demo: a `<TainoEditor>` with the full toolbar
//! (marks, link/image, headings, alignment, lists, blockquote, code-block,
//! case transforms, undo/redo, tables), the `Mod-…` keymap wired on
//! keydown, and live HTML + JSON panels. Build with `dx serve` (or
//! `trunk serve`) in this directory.
//!
//! The Dioxus adapter has full event- and plugin-wiring parity with the
//! Leptos one (input → transform, IME composition, paste, selectionchange,
//! plus `ViewPlugin` pointer events) — and this demo exercises the same
//! extension set as `basic-leptos`: type into the editor, click the
//! toolbar, drag across table cells, and watch the panels track the
//! document state.

use dioxus::prelude::*;
use taino_edit_dioxus::{
    set_block_type, toggle_mark, wrap_in, Attrs, Command, EditorState, KeymapProp, NodeSpec,
    SchemaBuilder, Selection, TainoEditor, Transaction, ViewPlugins,
};
use taino_edit_extensions::{
    add_column_after, add_row_after, align_center, align_justify, align_left, align_right,
    build_keymap_with, build_schema_with, delete_column, delete_row, delete_table, insert_image,
    insert_table, lift_list_item, merge_cells, redo_command, remove_link, select_caret_column,
    select_caret_row, set_column_width, set_link, split_cell, to_lowercase, to_uppercase,
    toggle_header_row, undo_command, wrap_in_bullet_list, wrap_in_ordered_list, Align, Blockquote,
    Bold, Code, CodeBlock, Heading, History, Image, Italic, Link, Lists, Paragraph, Table,
};
use taino_edit_table_view::TableView;

/// The extension set this demo runs on — identical to `basic-leptos`.
fn extensions() -> [&'static dyn taino_edit_extensions::Extension; 13] {
    [
        &Paragraph,
        &Heading,
        &Bold,
        &Italic,
        &Code,
        &Link,
        &Image,
        &Align,
        &Blockquote,
        &CodeBlock,
        &Lists,
        &Table,
        &History,
    ]
}

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
    let exts = extensions();
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
    let keymap = build_keymap_with(&extensions(), &schema, /*mac=*/ false);
    let state = use_signal(|| EditorState::new(doc, schema));

    // Toolbar buttons run a command and keep editor focus (mousedown
    // preventDefault stops the button from stealing the selection).
    let mark_cmd = move |mark: &str| {
        let schema = state.peek().schema().clone();
        if let Some(mt) = schema.mark_type(mark) {
            run_cmd(state, toggle_mark(mt.clone()));
        }
    };

    // Merge the caret's whole row / column. `select_caret_row` reads the
    // live table width, so this stays correct as the user grows the table
    // (the previous hard-coded `(0,0)-(0,2)` only worked for the original
    // 3×3 — a great way to ship the bug "merge only touches row 1 cols 1-3").
    let on_merge_row = move |_| {
        run_cmd(state, select_caret_row());
        run_cmd(state, merge_cells());
    };
    let on_merge_col = move |_| {
        run_cmd(state, select_caret_column());
        run_cmd(state, merge_cells());
    };

    let on_select_all = move |_| {
        let mut s = state;
        let snap = s.peek().clone();
        let mut tx = snap.tr();
        tx.set_selection(Selection::All);
        s.set(snap.apply(tx));
    };

    // Link command: ask the user for a URL, then dispatch set_link.
    let on_link = move |_| {
        let url = web_sys::window().and_then(|w| w.prompt_with_message("URL:").ok().flatten());
        if let Some(href) = url.filter(|s| !s.is_empty()) {
            run_cmd(state, set_link(href, None));
        }
    };

    // Image command: ask for a URL + alt text, then dispatch insert_image.
    let on_image = move |_| {
        let win = web_sys::window();
        let src = win
            .as_ref()
            .and_then(|w| w.prompt_with_message("Image URL:").ok().flatten())
            .filter(|s| !s.is_empty());
        let Some(src) = src else { return };
        let alt = win
            .as_ref()
            .and_then(|w| w.prompt_with_message("Alt text:").ok().flatten());
        run_cmd(state, insert_image(src, alt.filter(|s| !s.is_empty())));
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
                style: "display:flex; flex-wrap:wrap; gap:.4rem; margin-bottom:.5rem; align-items:center;",
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| mark_cmd("strong"),
                    "Bold"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| mark_cmd("em"),
                    "Italic"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| mark_cmd("code"),
                    "‹/› code"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: on_link,
                    "Link…"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, remove_link()),
                    "Unlink"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: on_image,
                    "Image…"
                }
                span { style: "width:.5rem" }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, set_block_type("paragraph", Attrs::new())),
                    "P"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, heading_cmd(1)),
                    "H1"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, heading_cmd(2)),
                    "H2"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, heading_cmd(3)),
                    "H3"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, wrap_in("blockquote", Attrs::new())),
                    "❝ Quote"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, set_block_type("code_block", Attrs::new())),
                    "<> Code"
                }
                span { style: "width:.5rem" }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, align_left()),
                    "⇤"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, align_center()),
                    "≡"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, align_right()),
                    "⇥"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, align_justify()),
                    "☰"
                }
                span { style: "width:.5rem" }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, wrap_in_bullet_list()),
                    "• List"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, wrap_in_ordered_list()),
                    "1. List"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, lift_list_item()),
                    "⇤ Lift"
                }
                span { style: "width:.5rem" }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, to_uppercase()),
                    "AA"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, to_lowercase()),
                    "aa"
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
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: on_select_all,
                    "Select all"
                }
            }

            div {
                role: "toolbar",
                style: "display:flex; flex-wrap:wrap; gap:.4rem; margin-bottom:.5rem; align-items:center;",
                strong { style: "font-size:.8rem; color:#555;", "Table:" }
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
                    onclick: move |_| run_cmd(state, delete_row()),
                    "− Row"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, delete_column()),
                    "− Col"
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
                    onclick: on_merge_col,
                    "Merge col"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, split_cell()),
                    "Split cell"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, set_column_width(0, 220)),
                    "Col wider"
                }
                button {
                    onmousedown: move |evt| evt.prevent_default(),
                    onclick: move |_| run_cmd(state, set_column_width(0, 80)),
                    "Col narrower"
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
