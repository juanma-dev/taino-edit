//! A textblock whose only tracked child is an inline **atom** (e.g. a
//! checkbox) — not text — must still detect text the browser inserts next
//! to that atom. `find_empty_block_text` originally only handled blocks
//! with *zero* tracked children; a block whose sole child is a non-text
//! atom fell through the same gap, since no `ViewDesc::Text` exists there
//! either to diff against (`collect_text_changes`) or to trigger the
//! empty-block path (`children.is_empty()` was false).
//!
//! Symptom before the fix: typing right after an atom is a real DOM text
//! node, genuinely inside the mounted element, but invisible to
//! `read_dom_changes()` — it's silently dropped until something else forces
//! a re-render, at which point the untracked text is orphaned in the DOM
//! and further typing lands at the wrong position.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{Attrs, DomSpec, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dom::EditorView;
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
        .node(
            "atom",
            NodeSpec {
                group: Some("inline".into()),
                inline: true,
                atom: true,
                to_dom: Some(|_| DomSpec::void("input").attr("type", "checkbox")),
                ..Default::default()
            },
        )
        .top_node("doc")
        .build()
        .unwrap()
}

fn doc(s: &Schema, ps: Vec<Node>) -> Node {
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

fn para(s: &Schema, kids: Vec<Node>) -> Node {
    s.node("paragraph", Default::default(), kids, vec![])
        .unwrap()
}

fn mount(d: Node, s: Schema) -> (EditorView, web_sys::Element) {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&root).unwrap();
    let view = EditorView::mount(d, s, root.clone());
    (view, root)
}

#[wasm_bindgen_test]
fn typing_after_an_atom_in_an_otherwise_empty_block_is_read_back() {
    let s = schema();
    let atom = s.node("atom", Attrs::new(), vec![], vec![]).unwrap();
    let (mut view, root) = mount(doc(&s, vec![para(&s, vec![atom.clone()])]), s.clone());

    // Simulate the browser inserting "hi" right after the atom's DOM node —
    // exactly what happens when the caret sits after a just-inserted
    // checkbox and the user starts typing the task label.
    let p = root.first_element_child().unwrap();
    let atom_dom = p.first_child().unwrap();
    let document = web_sys::window().unwrap().document().unwrap();
    let typed: Text = document.create_text_node("hi");
    p.insert_before(&typed, atom_dom.next_sibling().as_ref())
        .unwrap();

    let transform = view
        .read_dom_changes()
        .expect("typing next to the atom is detected");
    let text = s.text("hi", vec![]).unwrap();
    assert_eq!(
        transform.doc(),
        &doc(&s, vec![para(&s, vec![atom.clone(), text])]),
        "read-back inserts the typed text after the atom, not before it"
    );

    // Re-rendering from the model must not duplicate the typed text or drop
    // the atom.
    view.update(transform.doc().clone());
    let html = root.inner_html();
    assert!(
        root.query_selector("input").unwrap().is_some(),
        "the atom survives the re-render: {html}"
    );
    assert_eq!(
        root.query_selector("p").unwrap().unwrap().text_content(),
        Some("hi".to_string()),
        "no duplicated text after re-render: {html}"
    );
    let _ = root.parent_element().map(|b| b.remove_child(&root));
}
