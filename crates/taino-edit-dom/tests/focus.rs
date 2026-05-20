//! Phase 4 Unit H: focus management and tabindex.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{DomSpec, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dom::EditorView;
use wasm_bindgen_test::*;
use web_sys::Element;

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

fn attach(d: Node, s: Schema) -> (EditorView, Element) {
    let document = web_sys::window().unwrap().document().unwrap();
    let body = document.body().unwrap();
    let root = document.create_element("div").unwrap();
    body.append_child(&root).unwrap();
    let view = EditorView::mount(d, s, root.clone());
    (view, root)
}

fn cleanup(root: Element) {
    if let Some(parent) = root.parent_node() {
        let _ = parent.remove_child(&root);
    }
}

#[wasm_bindgen_test]
fn mount_sets_tabindex_zero_by_default() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "hi")]), s);
    assert_eq!(
        view.root().get_attribute("tabindex").as_deref(),
        Some("0"),
        "default tabindex puts editor in the Tab focus chain"
    );
    cleanup(root);
}

#[wasm_bindgen_test]
fn mount_respects_existing_tabindex() {
    let s = schema();
    let document = web_sys::window().unwrap().document().unwrap();
    let body = document.body().unwrap();
    let root = document.create_element("div").unwrap();
    root.set_attribute("tabindex", "-1").unwrap();
    body.append_child(&root).unwrap();

    let view = EditorView::mount(doc(&s, vec![para(&s, "hi")]), s, root.clone());
    assert_eq!(
        view.root().get_attribute("tabindex").as_deref(),
        Some("-1"),
        "explicit tabindex on root is preserved"
    );
    cleanup(root);
}

#[wasm_bindgen_test]
fn focus_makes_editor_the_active_element() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "hi")]), s);
    assert!(!view.has_focus());
    view.focus().unwrap();
    assert!(view.has_focus(), "after focus(), editor is active element");
    cleanup(root);
}

#[wasm_bindgen_test]
fn set_tabindex_updates_attribute() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "hi")]), s);
    view.set_tabindex(-1);
    assert_eq!(view.root().get_attribute("tabindex").as_deref(), Some("-1"));
    view.set_tabindex(0);
    assert_eq!(view.root().get_attribute("tabindex").as_deref(), Some("0"));
    cleanup(root);
}
