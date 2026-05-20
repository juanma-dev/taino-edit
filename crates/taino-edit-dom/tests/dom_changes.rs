//! Phase 4 Unit D: `EditorView::read_dom_changes` turns a real DOM-side text
//! edit (the effect of typing or IME commit) into a [`Transform`] that, when
//! applied, brings the document back in sync.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{DomSpec, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dom::EditorView;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::Text;

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

fn doc(s: &Schema, ps: Vec<Node>) -> Node {
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

fn make_view(doc: Node, s: Schema) -> EditorView {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    EditorView::mount(doc, s, root)
}

fn first_text(view: &EditorView) -> Text {
    let p = view.root().first_child().unwrap();
    p.first_child().unwrap().dyn_into().unwrap()
}

#[wasm_bindgen_test]
fn no_changes_returns_none() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "Hello")]), s);
    assert!(view.read_dom_changes().is_none());
}

#[wasm_bindgen_test]
fn typing_into_text_node_yields_correct_transform() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "Hello")]), s.clone());

    // Simulate the user typing `!` at the end of "Hello".
    let t = first_text(&view);
    t.set_data("Hello!");

    let transform = view.read_dom_changes().expect("changes detected");
    assert_eq!(transform.doc(), &doc(&s, vec![para(&s, "Hello!")]));
}

#[wasm_bindgen_test]
fn deleting_chars_works() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "Hello")]), s.clone());

    // Simulate the user backspacing two characters.
    let t = first_text(&view);
    t.set_data("Hel");

    let transform = view.read_dom_changes().unwrap();
    assert_eq!(transform.doc(), &doc(&s, vec![para(&s, "Hel")]));
}

#[wasm_bindgen_test]
fn change_in_second_block_uses_correct_doc_position() {
    let s = schema();
    let view = make_view(
        doc(&s, vec![para(&s, "first"), para(&s, "second")]),
        s.clone(),
    );

    // Edit the text inside the second paragraph.
    let p2 = view.root().last_child().unwrap();
    let t: Text = p2.first_child().unwrap().dyn_into().unwrap();
    t.set_data("secondX");

    let transform = view.read_dom_changes().unwrap();
    assert_eq!(
        transform.doc(),
        &doc(&s, vec![para(&s, "first"), para(&s, "secondX")])
    );
}

#[wasm_bindgen_test]
fn clearing_text_node_yields_empty_replace() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "x")]), s.clone());

    // Empty the text node entirely.
    let t = first_text(&view);
    t.set_data("");

    let transform = view.read_dom_changes().unwrap();
    // The new doc has the (now-empty) paragraph; the text node was removed.
    let expected_p = s
        .node("paragraph", Default::default(), vec![], vec![])
        .unwrap();
    let expected = doc(&s, vec![expected_p]);
    assert_eq!(transform.doc(), &expected);
}
