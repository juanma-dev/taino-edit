//! Phase 5 Unit A: `<TainoEditor>` mounts into a real Leptos render tree
//! in headless Chromium and reflects state-signal changes through the DOM.

#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use std::collections::HashMap;
use taino_edit_core::{
    AttrSpec, DomSpec, EditorState, Node, NodeSpec, Schema, SchemaBuilder, Selection,
};
use taino_edit_leptos::TainoEditor;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::HtmlElement;

/// Yield to the browser event loop so any Leptos effects deferred onto the
/// microtask queue can run before we read the DOM.
async fn settle() {
    TimeoutFuture::new(0).await;
}

wasm_bindgen_test_configure!(run_in_browser);

fn schema() -> Schema {
    SchemaBuilder::new()
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
            "heading",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                attrs: {
                    let mut m = HashMap::new();
                    m.insert(
                        "level".to_string(),
                        AttrSpec {
                            default: Some(serde_json::json!(1)),
                        },
                    );
                    m
                },
                to_dom: Some(|n| {
                    let level = n.attrs().get("level").and_then(|v| v.as_u64()).unwrap_or(1);
                    DomSpec::element(&format!("h{level}"))
                }),
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
        .top_node("doc")
        .build()
        .unwrap()
}

fn para(s: &Schema, t: &str) -> Node {
    s.node(
        "paragraph",
        Default::default(),
        vec![s.text(t, vec![]).unwrap()],
        vec![],
    )
    .unwrap()
}

fn doc(s: &Schema, ps: Vec<Node>) -> Node {
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

fn host() -> HtmlElement {
    let document = web_sys::window().unwrap().document().unwrap();
    let host = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&host).unwrap();
    host.unchecked_into()
}

#[wasm_bindgen_test]
async fn component_mounts_initial_document() {
    let host = host();
    let s = schema();
    let initial = EditorState::new(doc(&s, vec![para(&s, "Hello Leptos")]), s);

    leptos::mount::mount_to(host.clone(), move || {
        let state = RwSignal::new(initial.clone());
        view! { <TainoEditor state=state /> }
    })
    .forget();
    settle().await;

    let inner = host.inner_html();
    assert!(
        inner.contains("<p>Hello Leptos</p>"),
        "expected mounted document, got: {inner}"
    );
    assert!(
        inner.contains("contenteditable=\"true\""),
        "TainoEditor must enable contenteditable: {inner}"
    );
}

#[wasm_bindgen_test]
async fn signal_update_patches_the_dom() {
    let host = host();
    let s = schema();
    let state = RwSignal::new(EditorState::new(
        doc(&s, vec![para(&s, "before")]),
        s.clone(),
    ));

    leptos::mount::mount_to(host.clone(), move || view! { <TainoEditor state=state /> }).forget();
    settle().await;
    assert!(host.inner_html().contains("before"));

    // Change the state signal; the component reconciles the DOM.
    state.set(EditorState::new(doc(&s, vec![para(&s, "after")]), s));
    settle().await;
    assert!(
        host.inner_html().contains("after"),
        "after signal update, expected `after`, got: {}",
        host.inner_html()
    );
    assert!(!host.inner_html().contains("before"));
}

#[wasm_bindgen_test]
async fn state_with_selection_renders_blocks() {
    let host = host();
    let s = schema();
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("level".into(), serde_json::json!(2));
    let h = s
        .node(
            "heading",
            attrs,
            vec![s.text("Title", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    let st = EditorState::new(doc(&s, vec![h, para(&s, "body")]), s.clone());
    let _ = Selection::All; // smoke import — selection wiring lands in Unit B.

    leptos::mount::mount_to(host.clone(), move || {
        let state = RwSignal::new(st.clone());
        view! { <TainoEditor state=state /> }
    })
    .forget();
    settle().await;

    let inner = host.inner_html();
    assert!(
        inner.contains("<h2>Title</h2>"),
        "heading rendered: {inner}"
    );
    assert!(inner.contains("<p>body</p>"), "paragraph rendered: {inner}");
}
