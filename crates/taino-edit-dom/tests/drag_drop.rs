//! Phase 4 Unit I: drag-and-drop primitives — extract a Slice for the
//! dragged content, and drop it at a target position.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{DomSpec, MarkSpec, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dom::EditorView;
use wasm_bindgen_test::*;

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
        .mark(
            "strong",
            MarkSpec {
                to_dom: Some(|_| DomSpec::element("strong")),
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

fn make_view(d: Node, s: Schema) -> EditorView {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    EditorView::mount(d, s, root)
}

#[wasm_bindgen_test]
fn extract_then_drop_in_same_block_inserts_text() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "abcdef")]), s.clone());
    // Pull "cd" (positions 3..5).
    let slice = view.extract_slice(3, 5).unwrap();
    assert_eq!(slice.size(), 2);

    // Drop it at the end (position 7 = end of content).
    let transform = view.drop_slice(&slice, 7).unwrap();
    assert_eq!(transform.doc(), &doc(&s, vec![para(&s, "abcdefcd")]));
}

#[wasm_bindgen_test]
fn drop_into_inline_position_rejects_block_slice() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "Hello")]), s.clone());

    // Extract the whole paragraph (a block-level slice).
    let slice = view.extract_slice(0, 7).unwrap();
    // Try to drop it inside another paragraph's inline content (pos 3 is
    // mid-text). The schema rejects a block inside `inline*`.
    let dropped = view.drop_slice(&slice, 3);
    assert!(dropped.is_none());
}

#[wasm_bindgen_test]
fn extracted_slice_preserves_marks() {
    let s = schema();
    let strong = s.mark_type("strong").unwrap().create(Default::default());
    let plain = s.text("ab", vec![]).unwrap();
    let bold = s.text("cd", vec![strong]).unwrap();
    let p = s
        .node("paragraph", Default::default(), vec![plain, bold], vec![])
        .unwrap();
    let view = make_view(doc(&s, vec![p]), s.clone());

    // Extract the bold range "cd" (positions 3..5).
    let slice = view.extract_slice(3, 5).unwrap();
    let first = slice.content().child(0);
    assert_eq!(first.text(), Some("cd"));
    assert_eq!(first.marks().len(), 1);
    assert_eq!(first.marks()[0].mark_type().name(), "strong");
}

#[wasm_bindgen_test]
fn empty_extract_yields_empty_slice() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "ab")]), s);
    let slice = view.extract_slice(2, 2).unwrap();
    assert!(slice.is_empty());
}
