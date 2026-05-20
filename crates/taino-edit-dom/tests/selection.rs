//! Phase 4 Unit C: doc Selection ↔ `window.getSelection()` round-trip.
//! The view must be attached to `document.body` for the browser selection
//! API to operate on it.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{DomSpec, Node, NodeSpec, Schema, SchemaBuilder, Selection};
use taino_edit_dom::EditorView;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;
use web_sys::{Element, Text};

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
    let txt = s.text(t, vec![]).unwrap();
    s.node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap()
}

/// Attach a fresh `<div>` to `<body>` and return (view, cleanup).
fn attach(doc: Node, s: Schema) -> (EditorView, Element) {
    let document = web_sys::window().unwrap().document().unwrap();
    let body = document.body().unwrap();
    let root = document.create_element("div").unwrap();
    body.append_child(&root).unwrap();
    let view = EditorView::mount(doc, s, root.clone());
    (view, root)
}

fn cleanup(root: Element) {
    if let Some(parent) = root.parent_node() {
        let _ = parent.remove_child(&root);
    }
}

fn selection() -> web_sys::Selection {
    web_sys::window().unwrap().get_selection().unwrap().unwrap()
}

#[wasm_bindgen_test]
fn set_caret_in_text_writes_dom_selection() {
    let s = schema();
    let doc = s
        .node("doc", Default::default(), vec![para(&s, "Hello")], vec![])
        .unwrap();
    let (view, root) = attach(doc, s);

    // Inside "Hello", pos 3 = caret between 'e' and 'l' (text offset 2).
    view.set_selection(Selection::caret(3)).unwrap();

    let sel = selection();
    assert!(sel.is_collapsed());
    let anchor = sel.anchor_node().unwrap();
    let text: Text = anchor.dyn_into().unwrap();
    assert_eq!(text.data(), "Hello");
    assert_eq!(sel.anchor_offset(), 2);
    cleanup(root);
}

#[wasm_bindgen_test]
fn set_text_range_writes_anchor_and_focus() {
    let s = schema();
    let doc = s
        .node("doc", Default::default(), vec![para(&s, "Hello")], vec![])
        .unwrap();
    let (view, root) = attach(doc, s);

    view.set_selection(Selection::Text { anchor: 2, head: 5 })
        .unwrap();
    let sel = selection();
    assert!(!sel.is_collapsed());
    assert_eq!(sel.anchor_offset(), 1, "anchor at 'e'");
    assert_eq!(sel.focus_offset(), 4, "focus before 'o'");
    assert_eq!(sel.to_string().as_string().unwrap(), "ell");
    cleanup(root);
}

#[wasm_bindgen_test]
fn set_selection_all_covers_doc() {
    let s = schema();
    let doc = s
        .node(
            "doc",
            Default::default(),
            vec![para(&s, "ab"), para(&s, "cd")],
            vec![],
        )
        .unwrap();
    let (view, root) = attach(doc, s);

    view.set_selection(Selection::All).unwrap();
    // Anchor at root[0], focus at root[2] — i.e. before first and after last.
    let sel = selection();
    let anchor_root: JsValue = sel.anchor_node().unwrap().into();
    let view_root: JsValue = view.root().clone().into();
    assert!(anchor_root == view_root);
    assert_eq!(sel.anchor_offset(), 0);
    assert_eq!(sel.focus_offset(), 2);
    cleanup(root);
}

#[wasm_bindgen_test]
fn read_selection_after_setting_round_trips() {
    let s = schema();
    let doc = s
        .node(
            "doc",
            Default::default(),
            vec![para(&s, "Hello"), para(&s, "World")],
            vec![],
        )
        .unwrap();
    let (view, root) = attach(doc, s);

    // pos 4 = inside "Hello" between 'l' and 'l'; pos 10 = inside "World"
    // between 'W' and 'o'.
    let initial = Selection::Text {
        anchor: 4,
        head: 10,
    };
    view.set_selection(initial).unwrap();
    let back = view.read_selection().unwrap();
    assert_eq!(back, initial);
    cleanup(root);
}

#[wasm_bindgen_test]
fn position_boundary_between_blocks_maps_to_root() {
    let s = schema();
    let doc = s
        .node(
            "doc",
            Default::default(),
            vec![para(&s, "ab"), para(&s, "cd")],
            vec![],
        )
        .unwrap();
    let (view, root) = attach(doc, s);

    // Position 4 is the boundary between the two paragraphs at the doc
    // level (after p1, before p2).
    view.set_selection(Selection::caret(4)).unwrap();
    let sel = selection();
    let anchor: JsValue = sel.anchor_node().unwrap().into();
    let r: JsValue = view.root().clone().into();
    assert!(anchor == r);
    assert_eq!(sel.anchor_offset(), 1, "between the two paragraphs");
    cleanup(root);
}

#[wasm_bindgen_test]
fn read_selection_when_dom_selection_is_outside_returns_none() {
    let s = schema();
    let doc = s
        .node("doc", Default::default(), vec![para(&s, "x")], vec![])
        .unwrap();
    let (view, root) = attach(doc, s);

    // Put the DOM selection on a node not inside this view.
    let document = web_sys::window().unwrap().document().unwrap();
    let outside = document.create_element("span").unwrap();
    document.body().unwrap().append_child(&outside).unwrap();
    outside.set_text_content(Some("nope"));
    let range = document.create_range().unwrap();
    range.select_node_contents(&outside).unwrap();
    let sel = selection();
    sel.remove_all_ranges().unwrap();
    sel.add_range(&range).unwrap();

    assert!(view.read_selection().is_none());
    let _ = document.body().unwrap().remove_child(&outside);
    cleanup(root);
}
