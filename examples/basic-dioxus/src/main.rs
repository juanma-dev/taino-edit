//! Minimal taino-edit + Dioxus demo: a `<TainoEditor>` over a Dioxus
//! `Signal<EditorState>`. Build with `dx serve` in this directory.
//!
//! v0.2 of the Dioxus adapter is a proof-of-architecture: mount + DOM
//! patching work end-to-end. Full event-wiring parity with the Leptos
//! adapter (input → transform, IME, paste, selectionchange) ships in
//! v0.2.x — the `taino-edit-dom` pieces it needs are already in place.

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
                "v0.2 Dioxus adapter: mount + DOM patching prove the architecture. "
                "Full event wiring lands in v0.2.x — for production today, see the Leptos demo."
            }
            TainoEditor { state }
        }
    }
}
