//! Phase 4 Unit G: node decorations apply / remove CSS classes on block
//! elements without touching the document.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{DomSpec, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dom::{Decoration, EditorView};
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

fn make_view(doc: Node, s: Schema) -> EditorView {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    EditorView::mount(doc, s, root)
}

fn block_at(view: &EditorView, idx: usize) -> web_sys::Element {
    use wasm_bindgen::JsCast;
    let nodes = view.root().child_nodes();
    nodes.item(idx as u32).unwrap().dyn_into().unwrap()
}

#[wasm_bindgen_test]
fn set_node_decoration_adds_class_to_block() {
    let s = schema();
    let mut view = make_view(doc(&s, vec![para(&s, "a"), para(&s, "b")]), s);

    // Doc positions: first paragraph starts at 0, second at 3.
    view.set_decorations(vec![Decoration::Node {
        pos: 3,
        class: "search-hit".into(),
    }]);

    assert!(!block_at(&view, 0).class_list().contains("search-hit"));
    assert!(block_at(&view, 1).class_list().contains("search-hit"));
    assert_eq!(view.decorations().len(), 1);
}

#[wasm_bindgen_test]
fn replacing_decorations_removes_the_old_ones() {
    let s = schema();
    let mut view = make_view(doc(&s, vec![para(&s, "a"), para(&s, "b")]), s);

    view.set_decorations(vec![Decoration::Node {
        pos: 0,
        class: "first".into(),
    }]);
    assert!(block_at(&view, 0).class_list().contains("first"));

    view.set_decorations(vec![Decoration::Node {
        pos: 3,
        class: "second".into(),
    }]);
    assert!(!block_at(&view, 0).class_list().contains("first"));
    assert!(block_at(&view, 1).class_list().contains("second"));
}

#[wasm_bindgen_test]
fn clearing_decorations_removes_all_classes() {
    let s = schema();
    let mut view = make_view(doc(&s, vec![para(&s, "a")]), s);

    view.set_decorations(vec![Decoration::Node {
        pos: 0,
        class: "x".into(),
    }]);
    assert!(block_at(&view, 0).class_list().contains("x"));
    view.set_decorations(Vec::new());
    assert!(!block_at(&view, 0).class_list().contains("x"));
    assert!(view.decorations().is_empty());
}

#[wasm_bindgen_test]
fn out_of_range_decoration_is_silently_skipped() {
    let s = schema();
    let mut view = make_view(doc(&s, vec![para(&s, "a")]), s);
    // pos 100 is way past doc.content.size(); no block to decorate, but
    // set_decorations must not panic.
    view.set_decorations(vec![Decoration::Node {
        pos: 100,
        class: "ghost".into(),
    }]);
    assert!(!block_at(&view, 0).class_list().contains("ghost"));
}
