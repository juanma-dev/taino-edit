//! taino-edit + Dioxus demo: a `<TainoEditor>` with a toolbar, the
//! `Mod-…` keymap wired on keydown, and live HTML + JSON panels. Build
//! with `dx serve` (or `trunk serve`) in this directory.
//!
//! The Dioxus adapter has full event-wiring parity with the Leptos one
//! (input → transform, IME composition, paste, selectionchange) — type
//! into the editor, click the toolbar, and watch the panels track the
//! document state.

use dioxus::prelude::*;
use taino_edit_dioxus::{
    set_block_type, toggle_mark, Attrs, Command, EditorState, KeyPress, NodeSpec, SchemaBuilder,
    TainoEditor, Transaction,
};
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, redo_command, undo_command, Bold, Heading, History,
    Italic, Paragraph,
};

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
        vec![&Paragraph, &Heading, &Bold, &Italic, &History];
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
        .text("Type, format, undo — every command goes through Rust.", vec![])
        .unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![body], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![h, p], vec![])
        .unwrap();

    let schema_for_keymap = schema.clone();
    let mut state = use_signal(|| EditorState::new(doc, schema));
    // Keymap (Mod-b / Mod-i / Mod-Alt-0..3 / Mod-z / Mod-Shift-z). Built
    // once; the closure re-creates the extension values locally so their
    // borrows only need to live for the build call.
    let keymap = use_signal(move || {
        let exts: Vec<&dyn taino_edit_extensions::Extension> =
            vec![&Paragraph, &Heading, &Bold, &Italic, &History];
        build_keymap_with(&exts, &schema_for_keymap, /*mac=*/ false)
    });

    // Toolbar buttons run a command and keep editor focus (mousedown
    // preventDefault stops the button from stealing the selection).
    let mark_cmd = move |mark: &str| {
        let schema = state.peek().schema().clone();
        if let Some(mt) = schema.mark_type(mark) {
            run_cmd(state, toggle_mark(mt.clone()));
        }
    };

    let on_keydown = move |evt: KeyboardEvent| {
        let mods = evt.modifiers();
        let key = KeyPress {
            key: evt.key().to_string(),
            ctrl: mods.contains(Modifiers::CONTROL),
            alt: mods.contains(Modifiers::ALT),
            shift: mods.contains(Modifiers::SHIFT),
            meta: mods.contains(Modifiers::META),
        };
        let snap = state.peek().clone();
        let mut next = None;
        let handled = {
            let km = keymap.peek();
            km.handle(&snap, &key, Some(&mut |tx: Transaction| next = Some(snap.apply(tx))))
        };
        if let Some(n) = next {
            state.set(n);
        }
        if handled {
            evt.prevent_default();
        }
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
            }

            div {
                onkeydown: on_keydown,
                TainoEditor { state }
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
