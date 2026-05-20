//! Phase 4 Unit F: IME composition lifecycle. While composing, transient
//! DOM glyph states must not produce transactions; on commit, the final
//! composed text becomes a single transform.

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

fn first_text(view: &EditorView) -> Text {
    let p = view.root().first_child().unwrap();
    p.first_child().unwrap().dyn_into().unwrap()
}

#[wasm_bindgen_test]
fn composing_flag_starts_false_and_toggles() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "x")]), s);
    assert!(!view.is_composing());
    view.composition_start();
    assert!(view.is_composing());
    view.composition_end();
    assert!(!view.is_composing());
}

#[wasm_bindgen_test]
fn changes_during_composition_are_suppressed() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "Hello")]), s.clone());

    view.composition_start();

    // Simulate the IME repeatedly rewriting the text node with transient
    // intermediate glyphs (think Japanese romaji → kana). None of these
    // must surface as a transaction.
    let t = first_text(&view);
    for intermediate in ["Hellou", "Helloux", "こんにちは"] {
        t.set_data(intermediate);
        assert!(
            view.read_dom_changes().is_none(),
            "intermediate `{intermediate}` must not produce a transform"
        );
    }
}

#[wasm_bindgen_test]
fn composition_end_releases_the_lock_and_change_commits() {
    let s = schema();
    let view = make_view(doc(&s, vec![para(&s, "Hello")]), s.clone());

    view.composition_start();
    let t = first_text(&view);
    t.set_data("こんにちは"); // committed glyphs
    assert!(view.read_dom_changes().is_none(), "still composing");

    view.composition_end();
    let transform = view.read_dom_changes().expect("commit yields transform");
    assert_eq!(transform.doc(), &doc(&s, vec![para(&s, "こんにちは")]));
}
