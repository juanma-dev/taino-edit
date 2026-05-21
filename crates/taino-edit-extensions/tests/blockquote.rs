//! Phase 6 Unit G: the `Blockquote` extension.

use taino_edit_core::{EditorState, KeyPress, NodeSpec, SchemaBuilder, Selection};
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, Blockquote, Extension, Paragraph,
};

fn make_state() -> EditorState {
    let base = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        );
    let schema = build_schema_with(base, &[&Paragraph, &Blockquote], "doc").unwrap();
    let txt = schema.text("hello", vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    EditorState::new(doc, schema)
}

#[test]
fn blockquote_registers_node_and_binding() {
    assert_eq!(Blockquote.name(), "blockquote");
    let adds = Blockquote.schema_additions();
    assert_eq!(adds.nodes.len(), 1);
    assert_eq!(adds.nodes[0].0, "blockquote");

    let s = make_state();
    let bindings = Blockquote.keymap_entries(s.schema());
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "Mod->");
}

#[test]
fn mod_gt_wraps_paragraph_in_blockquote() {
    let s = make_state();
    let mut t = s.tr();
    t.set_selection(Selection::caret(2));
    let s = s.apply(t);

    let keymap = build_keymap_with(&[&Paragraph, &Blockquote], s.schema(), false);
    let mut next = None;
    {
        let mut d = |tx| next = Some(s.apply(tx));
        let handled = keymap.handle(&s, &KeyPress::key(">").ctrl().shift(), Some(&mut d));
        assert!(handled, "Mod-> must be bound to wrap-in-blockquote");
    }
    let s2 = next.expect("dispatch fired");
    let html = s2.doc().to_html();
    assert!(
        html.contains("<blockquote><p>hello</p></blockquote>"),
        "expected blockquote wrapping the paragraph, got: {html}"
    );
}
