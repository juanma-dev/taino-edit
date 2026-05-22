//! The `Code` inline-mark extension.

use taino_edit_core::{EditorState, KeyPress, NodeSpec, SchemaBuilder, Selection};
use taino_edit_extensions::{build_keymap_with, build_schema_with, Code, Extension, Paragraph};

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
    let schema = build_schema_with(base, &[&Paragraph, &Code], "doc").unwrap();
    let txt = schema.text("hello world", vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    EditorState::new(doc, schema)
}

#[test]
fn code_contributes_mark_and_binding() {
    assert_eq!(Code.name(), "code");
    let adds = Code.schema_additions();
    assert!(adds.nodes.is_empty());
    assert_eq!(adds.marks.len(), 1);
    let (name, spec) = &adds.marks[0];
    assert_eq!(name, "code");
    assert!(!spec.inclusive, "code spans must not extend on typing");

    let s = make_state();
    let bindings = Code.keymap_entries(s.schema());
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "Mod-e");
}

#[test]
fn mod_e_toggles_code_on_a_selection() {
    let s = make_state();
    let mut t = s.tr();
    t.set_selection(Selection::Text { anchor: 1, head: 6 }); // "hello"
    let s = s.apply(t);

    let keymap = build_keymap_with(&[&Paragraph, &Code], s.schema(), false);
    let mut next = None;
    {
        let mut d = |tx| next = Some(s.apply(tx));
        let handled = keymap.handle(&s, &KeyPress::key("e").ctrl(), Some(&mut d));
        assert!(handled, "Mod-e must be bound to toggle code");
    }
    let s2 = next.expect("dispatch fired");
    assert!(
        s2.doc().to_html().contains("<code>hello</code>"),
        "expected inline code in HTML: {}",
        s2.doc().to_html()
    );
}

#[test]
fn code_parses_back_from_html() {
    let s = make_state();
    let parsed = s
        .schema()
        .parse_html("<p>see <code>x = 1</code> there</p>")
        .unwrap();
    let html = parsed.to_html();
    assert!(
        html.contains("<code>x = 1</code>"),
        "code mark round-trips: {html}"
    );
}
