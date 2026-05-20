//! Phase 4 Unit E: clipboard paste — plain text and HTML, the latter
//! sanitized through `Schema::parse_html` so untrusted clipboard content
//! cannot inject schema-illegal structure.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{DomSpec, Node, NodeSpec, ParseRule, Schema, SchemaBuilder, Selection};
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
                parse_dom: vec![ParseRule::tag("p")],
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

#[wasm_bindgen_test]
fn paste_text_at_caret_inserts() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "Hello")]), s.clone());

    // Caret at pos 3 (between 'e' and 'l').
    view.set_selection(Selection::caret(3)).unwrap();
    let transform = view.paste_text("XYZ").expect("transform produced");
    assert_eq!(transform.doc(), &doc(&s, vec![para(&s, "HeXYZllo")]));
    cleanup(root);
}

#[wasm_bindgen_test]
fn paste_text_over_range_replaces() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "Hello")]), s.clone());

    // Select "ell" (positions 2..5).
    view.set_selection(Selection::Text { anchor: 2, head: 5 })
        .unwrap();
    let transform = view.paste_text("ELL").unwrap();
    assert_eq!(transform.doc(), &doc(&s, vec![para(&s, "HELLo")]));
    cleanup(root);
}

#[wasm_bindgen_test]
fn paste_text_with_empty_string_deletes_selection() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "Hello")]), s.clone());

    view.set_selection(Selection::Text { anchor: 2, head: 5 })
        .unwrap();
    let transform = view.paste_text("").unwrap();
    assert_eq!(transform.doc(), &doc(&s, vec![para(&s, "Ho")]));
    cleanup(root);
}

#[wasm_bindgen_test]
fn paste_html_known_tags_inserts_blocks() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "x")]), s.clone());

    // Caret at the boundary between blocks (doc pos 3 = after the only
    // paragraph) so a fresh paragraph fits cleanly.
    view.set_selection(Selection::caret(3)).unwrap();
    let transform = view
        .paste_html("<p>pasted</p>")
        .expect("html paste produces a transform");
    assert_eq!(
        transform.doc(),
        &doc(&s, vec![para(&s, "x"), para(&s, "pasted")])
    );
    cleanup(root);
}

#[wasm_bindgen_test]
fn paste_html_strips_script_tags() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "ok")]), s.clone());
    view.set_selection(Selection::caret(4)).unwrap();

    // <script> is not in the schema's parse_dom rules → its element is
    // unwrapped by parse_html; the surviving text is what could go in.
    // Here the whole payload becomes nothing schema-valid for a block-level
    // paste, so the transform is rejected.
    let result = view.paste_html("<script>alert('x')</script>");
    // Either None (rejected) or, if accepted, must NOT contain literal markup.
    if let Some(t) = result {
        assert!(
            !t.doc().text_content().contains("alert"),
            "script payload must never make it into the document"
        );
    }
    cleanup(root);
}

#[wasm_bindgen_test]
fn paste_html_invalid_in_context_returns_none() {
    let s = schema();
    let (view, root) = attach(doc(&s, vec![para(&s, "Hello")]), s.clone());
    // Caret inside the paragraph (inline content); pasting a <p> there
    // would put a block inside `inline*`, which the schema rejects.
    view.set_selection(Selection::caret(3)).unwrap();
    assert!(view.paste_html("<p>x</p>").is_none());
    cleanup(root);
}
