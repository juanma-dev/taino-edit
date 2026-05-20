//! A minimal `<TainoEditor>` demo: build with `trunk serve`.

use leptos::prelude::*;
use taino_edit_leptos::{
    toggle_mark, DomSpec, EditorState, MarkSpec, NodeSpec, SchemaBuilder, TainoEditor,
};

fn main() {
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let schema = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "paragraph",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                to_dom: Some(|_| DomSpec::element("p")),
                ..Default::default()
            },
        )
        .node(
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        )
        .mark(
            "strong",
            MarkSpec {
                to_dom: Some(|_| DomSpec::element("strong")),
                ..Default::default()
            },
        )
        .top_node("doc")
        .build()
        .expect("schema builds");

    let initial_text = schema.text("Edita aquí…", vec![]).unwrap();
    let initial_para = schema
        .node("paragraph", Default::default(), vec![initial_text], vec![])
        .unwrap();
    let initial_doc = schema
        .node("doc", Default::default(), vec![initial_para], vec![])
        .unwrap();
    let state = RwSignal::new(EditorState::new(initial_doc, schema.clone()));

    let strong = schema.mark_type("strong").unwrap().clone();
    let bold_cmd = toggle_mark(strong);
    let on_bold = move |_| {
        let snapshot = state.get_untracked();
        let mut new_state = None;
        {
            let mut dispatch = |tx| new_state = Some(snapshot.apply(tx));
            bold_cmd(&snapshot, Some(&mut dispatch));
        }
        if let Some(next) = new_state {
            state.set(next);
        }
    };

    let on_undo = move |_| {
        if let Some(next) = state.get_untracked().undo() {
            state.set(next);
        }
    };
    let on_redo = move |_| {
        if let Some(next) = state.get_untracked().redo() {
            state.set(next);
        }
    };

    view! {
        <main style="font-family: system-ui; max-width: 40rem; margin: 2rem auto;">
            <h1>"taino-edit demo"</h1>
            <p>"A pure-Rust WYSIWYG editor mounted as a Leptos component."</p>
            <div style="display:flex; gap:.5rem; margin-bottom:.5rem;">
                <button on:click=on_bold>"Bold"</button>
                <button on:click=on_undo>"Undo"</button>
                <button on:click=on_redo>"Redo"</button>
            </div>
            <TainoEditor state=state />
        </main>
    }
}
