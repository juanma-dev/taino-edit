//! taino-edit + Dioxus demo: a `<TainoEditor>` over a Dioxus
//! `Signal<EditorState>`, with live HTML + JSON panels. Build with
//! `dx serve` (or `trunk serve`) in this directory.
//!
//! The Dioxus adapter has full event-wiring parity with the Leptos one
//! (input → transform, IME composition, paste, selectionchange) — type
//! into the editor and watch the panels track the document state.

use dioxus::prelude::*;
use taino_edit_dioxus::{Attrs, EditorState, NodeSpec, SchemaBuilder, TainoEditor};
use taino_edit_extensions::{build_schema_with, Bold, Heading, Italic, Paragraph};

fn main() {
    dioxus::launch(App);
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
        vec![&Paragraph, &Heading, &Bold, &Italic];
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
        .text("Pure-Rust WYSIWYG, no JS bridge.", vec![])
        .unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![body], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![h, p], vec![])
        .unwrap();
    let state = use_signal(|| EditorState::new(doc, schema));

    rsx! {
        main {
            style: "font-family: system-ui; max-width: 60rem; margin: 1.5rem auto; padding: 0 1rem;",
            h1 { "taino-edit + Dioxus demo" }
            p {
                style: "color:#555;",
                "Pure-Rust WYSIWYG running in Dioxus. Type below — the live HTML and "
                "JSON panels track every edit through the same transforms as the Leptos build."
            }
            TainoEditor { state }
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
