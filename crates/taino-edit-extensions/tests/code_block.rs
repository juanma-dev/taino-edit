//! Phase 6 Unit H: the `CodeBlock` extension.

use taino_edit_core::{EditorState, KeyPress, NodeSpec, SchemaBuilder, Selection};
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, CodeBlock, Extension, Paragraph,
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
    let schema = build_schema_with(base, &[&Paragraph, &CodeBlock], "doc").unwrap();
    let txt = schema.text("fn main()", vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    EditorState::new(doc, schema)
}

#[test]
fn code_block_registers_node_and_binding() {
    assert_eq!(CodeBlock.name(), "code_block");
    let adds = CodeBlock.schema_additions();
    assert_eq!(adds.nodes.len(), 1);
    assert_eq!(adds.nodes[0].0, "code_block");

    let s = make_state();
    let bindings = CodeBlock.keymap_entries(s.schema());
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "Mod-`");
}

#[test]
fn mod_backtick_turns_paragraph_into_code_block() {
    let s = make_state();
    let mut t = s.tr();
    t.set_selection(Selection::caret(3));
    let s = s.apply(t);

    let keymap = build_keymap_with(&[&Paragraph, &CodeBlock], s.schema(), false);
    let mut next = None;
    {
        let mut d = |tx| next = Some(s.apply(tx));
        let handled = keymap.handle(&s, &KeyPress::key("`").ctrl(), Some(&mut d));
        assert!(handled, "Mod-` must be bound to set-block-type code_block");
    }
    let s2 = next.expect("dispatch fired");
    assert_eq!(s2.doc().child(0).node_type().name(), "code_block");
    let html = s2.doc().to_html();
    assert!(
        html.contains("<pre>fn main()</pre>"),
        "code_block should serialize as <pre>…</pre>, got: {html}"
    );
}

#[test]
fn code_block_parses_back_from_html() {
    let s = make_state();
    let html = "<pre>let x = 1;</pre>";
    let parsed = s.schema().parse_html(html).expect("parse");
    assert_eq!(parsed.child(0).node_type().name(), "code_block");
    assert_eq!(parsed.text_content(), "let x = 1;");
}
