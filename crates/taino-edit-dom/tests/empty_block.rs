//! Empty textblocks must be focusable and typable: they render a trailing
//! `<br>` (a bare `<p></p>` is zero-height in `contenteditable`), and the
//! read-back detects text the browser inserts into a previously-empty block
//! (e.g. typing into the paragraph created by pressing Enter).

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{DomSpec, EditorState, Node, NodeSpec, Schema, SchemaBuilder, Selection};
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
        .top_node("doc")
        .build()
        .unwrap()
}

fn para(s: &Schema, t: &str) -> Node {
    let kids = if t.is_empty() {
        vec![]
    } else {
        vec![s.text(t, vec![]).unwrap()]
    };
    s.node("paragraph", Default::default(), kids, vec![])
        .unwrap()
}

fn doc(s: &Schema, ps: Vec<Node>) -> Node {
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

fn mount(d: Node, s: Schema) -> (EditorView, web_sys::Element) {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&root).unwrap();
    let view = EditorView::mount(d, s, root.clone());
    (view, root)
}

#[wasm_bindgen_test]
fn empty_paragraph_renders_a_trailing_break() {
    let s = schema();
    let (_view, root) = mount(doc(&s, vec![para(&s, "")]), s);
    let html = root.inner_html();
    assert!(
        html.contains("<br"),
        "empty paragraph needs a trailing <br> to be focusable: {html:?}"
    );
    let _ = root.parent_element().map(|b| b.remove_child(&root));
}

#[wasm_bindgen_test]
fn filling_then_emptying_toggles_the_break() {
    let s = schema();
    let (mut view, root) = mount(doc(&s, vec![para(&s, "hi")]), s.clone());
    // Non-empty: no trailing break.
    assert!(!root.inner_html().contains("<br"), "{}", root.inner_html());

    // Empty it: the break appears.
    view.update(doc(&s, vec![para(&s, "")]));
    assert!(
        root.inner_html().contains("<br"),
        "emptying a block should add the break: {}",
        root.inner_html()
    );

    // Refill it: the break goes away and there's no duplicate text.
    view.update(doc(&s, vec![para(&s, "back")]));
    let html = root.inner_html();
    assert!(!html.contains("<br"), "refilling removes the break: {html}");
    assert_eq!(
        root.query_selector("p")
            .unwrap()
            .unwrap()
            .text_content()
            .as_deref(),
        Some("back")
    );
    let _ = root.parent_element().map(|b| b.remove_child(&root));
}

#[wasm_bindgen_test]
fn split_at_end_yields_a_focusable_empty_block() {
    let s = schema();
    // caret at end of "hi" (pos 3: p@0, text 1..3).
    let st = EditorState::new(doc(&s, vec![para(&s, "hi")]), s.clone());
    let mut tx = st.tr();
    tx.set_selection(Selection::caret(3));
    let st = st.apply(tx);

    let mut next = None;
    {
        let mut d = |t: taino_edit_core::Transaction| next = Some(st.apply(t));
        taino_edit_core::split_block(&st, Some(&mut d));
    }
    let after = next.expect("split dispatched");

    let (mut view, root) = mount(st.doc().clone(), s);
    view.update(after.doc().clone());
    let html = root.inner_html();
    assert_eq!(
        root.query_selector_all("p").unwrap().length(),
        2,
        "split yields two paragraphs: {html}"
    );
    assert!(
        html.contains("<br"),
        "the empty trailing paragraph needs a <br>: {html}"
    );
    let _ = root.parent_element().map(|b| b.remove_child(&root));
}

#[wasm_bindgen_test]
fn typing_into_an_empty_block_is_read_back_without_duplication() {
    let s = schema();
    // doc [p("hi"), p("")]; the empty p's content position is 5.
    let (mut view, root) = mount(doc(&s, vec![para(&s, "hi"), para(&s, "")]), s.clone());

    // Simulate the browser inserting "x" into the empty second paragraph
    // (before its trailing <br>).
    let p1 = root.last_element_child().unwrap();
    let document = web_sys::window().unwrap().document().unwrap();
    let typed: Text = document.create_text_node("x");
    let br = p1.first_child().unwrap();
    p1.insert_before(&typed, Some(&br)).unwrap();

    // The read-back turns that into an insert at the empty block.
    let transform = view
        .read_dom_changes()
        .expect("typing into the empty block is detected");
    assert_eq!(
        transform.doc(),
        &doc(&s, vec![para(&s, "hi"), para(&s, "x")]),
        "read-back inserts the typed text into the empty block"
    );

    // Re-rendering from the model must not duplicate the typed text.
    view.update(transform.doc().clone());
    let html = root.inner_html();
    let p1_after = root.last_element_child().unwrap();
    assert_eq!(
        p1_after.text_content().as_deref(),
        Some("x"),
        "no duplicated text after re-render: {html}"
    );
    assert!(
        !html.contains("<br"),
        "the now-filled block drops its break: {html}"
    );
    let _ = root.parent_element().map(|b| b.remove_child(&root));
}
