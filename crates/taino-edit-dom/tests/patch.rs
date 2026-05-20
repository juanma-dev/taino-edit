//! Phase 4 Unit B: `EditorView::update` patches the DOM incrementally —
//! identical subtrees keep their nodes, text-only changes reuse the same
//! text node, and only nodes that truly differ are replaced/removed/added.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use serde_json::json;
use taino_edit_core::{AttrSpec, DomSpec, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dom::EditorView;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;
use web_sys::{HtmlElement, Text};

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
                            default: Some(json!(1)),
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
    let txt = s.text(t, vec![]).unwrap();
    s.node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap()
}

fn doc(s: &Schema, ps: Vec<Node>) -> Node {
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

fn make_view(doc: Node, s: Schema) -> EditorView {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    EditorView::mount(doc, s, root)
}

fn inner_html(view: &EditorView) -> String {
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    html.inner_html()
}

#[wasm_bindgen_test]
fn update_with_identical_doc_is_a_noop() {
    let s = schema();
    let d1 = doc(&s, vec![para(&s, "Hello")]);
    let d2 = doc(&s, vec![para(&s, "Hello")]); // structurally identical
    let mut view = make_view(d1, s);

    // Capture identity of the rendered <p>.
    let before: JsValue = view.root().first_child().unwrap().into();
    view.update(d2);
    let after: JsValue = view.root().first_child().unwrap().into();

    assert!(
        before == after,
        "identical docs must not churn DOM elements"
    );
    assert_eq!(inner_html(&view), "<p>Hello</p>");
}

#[wasm_bindgen_test]
fn update_text_in_place_keeps_the_text_node() {
    let s = schema();
    let d1 = doc(&s, vec![para(&s, "Hello")]);
    let d2 = doc(&s, vec![para(&s, "Hello world")]);
    let mut view = make_view(d1, s);

    let p = view.root().first_child().unwrap();
    let text_before: JsValue = p.first_child().unwrap().into();
    view.update(d2);
    let p_after = view.root().first_child().unwrap();
    let text_after: JsValue = p_after.first_child().unwrap().into();

    assert!(text_before == text_after, "same Text DOM node reused");
    let t: Text = text_after.dyn_into().unwrap();
    assert_eq!(t.data(), "Hello world");
}

#[wasm_bindgen_test]
fn update_appends_new_block() {
    let s = schema();
    let d1 = doc(&s, vec![para(&s, "a")]);
    let d2 = doc(&s, vec![para(&s, "a"), para(&s, "b")]);
    let mut view = make_view(d1, s);

    let first_before: JsValue = view.root().first_child().unwrap().into();
    view.update(d2);
    let first_after: JsValue = view.root().first_child().unwrap().into();

    assert!(first_before == first_after, "existing first block reused");
    assert_eq!(inner_html(&view), "<p>a</p><p>b</p>");
    assert_eq!(view.children().len(), 2);
}

#[wasm_bindgen_test]
fn update_removes_trailing_block() {
    let s = schema();
    let d1 = doc(&s, vec![para(&s, "a"), para(&s, "b")]);
    let d2 = doc(&s, vec![para(&s, "a")]);
    let mut view = make_view(d1, s);

    view.update(d2);
    assert_eq!(inner_html(&view), "<p>a</p>");
    assert_eq!(view.children().len(), 1);
}

#[wasm_bindgen_test]
fn update_replaces_when_node_type_changes() {
    let s = schema();
    let d1 = doc(&s, vec![para(&s, "Title")]);
    // Same content, different block type → must replace the element.
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("level".into(), json!(2));
    let h = s
        .node(
            "heading",
            attrs,
            vec![s.text("Title", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    let d2 = doc(&s, vec![h]);
    let mut view = make_view(d1, s);

    let before: JsValue = view.root().first_child().unwrap().into();
    view.update(d2);
    let after: JsValue = view.root().first_child().unwrap().into();

    assert!(
        before != after,
        "different node type must replace the element"
    );
    assert_eq!(inner_html(&view), "<h2>Title</h2>");
}

#[wasm_bindgen_test]
fn update_recurses_into_nested_children() {
    let s = schema();
    let d1 = doc(&s, vec![para(&s, "x"), para(&s, "y")]);
    let d2 = doc(&s, vec![para(&s, "X"), para(&s, "y")]); // only first p's text changed
    let mut view = make_view(d1, s);

    let p1_before: JsValue = view.root().first_child().unwrap().into();
    let p2_before: JsValue = view
        .root()
        .first_child()
        .unwrap()
        .next_sibling()
        .unwrap()
        .into();
    view.update(d2);
    let p1_after: JsValue = view.root().first_child().unwrap().into();
    let p2_after: JsValue = view
        .root()
        .first_child()
        .unwrap()
        .next_sibling()
        .unwrap()
        .into();

    assert!(p1_before == p1_after, "first <p> element reused");
    assert!(p2_before == p2_after, "second <p> element reused");
    assert_eq!(inner_html(&view), "<p>X</p><p>y</p>");
}
