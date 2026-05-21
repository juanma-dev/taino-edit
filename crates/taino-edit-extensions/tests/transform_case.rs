//! Phase 6 Unit F: `to_uppercase` / `to_lowercase` commands.

use taino_edit_core::{Command, EditorState, NodeSpec, SchemaBuilder, Selection};
use taino_edit_extensions::{
    build_schema_with, to_lowercase, to_uppercase, Bold, Extension, Paragraph, TransformCase,
};

fn run(state: EditorState, cmd: &Command) -> EditorState {
    let mut next = None;
    {
        let mut d = |tx| next = Some(state.apply(tx));
        cmd(&state, Some(&mut d));
    }
    next.unwrap_or(state)
}

fn make_state(text: &str) -> EditorState {
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
    let schema = build_schema_with(base, &[&Paragraph, &Bold, &TransformCase], "doc").unwrap();
    let txt = schema.text(text, vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    EditorState::new(doc, schema)
}

#[test]
fn transform_case_contributes_nothing_to_schema() {
    let adds = TransformCase.schema_additions();
    assert!(adds.nodes.is_empty());
    assert!(adds.marks.is_empty());
}

#[test]
fn to_uppercase_transforms_selection_text() {
    let s = make_state("hello world");
    let mut t = s.tr();
    t.set_selection(Selection::Text { anchor: 1, head: 6 }); // "hello"
    let s = s.apply(t);

    let s = run(s, &to_uppercase());
    assert_eq!(s.doc().text_content(), "HELLO world");
}

#[test]
fn to_lowercase_transforms_selection_text() {
    let s = make_state("HELLO WORLD");
    let mut t = s.tr();
    t.set_selection(Selection::Text {
        anchor: 1,
        head: 12,
    });
    let s = s.apply(t);

    let s = run(s, &to_lowercase());
    assert_eq!(s.doc().text_content(), "hello world");
}

#[test]
fn case_commands_preserve_marks() {
    // "h<strong>ello</strong> world" — mark on "ello" (positions 2..5).
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
    let schema = build_schema_with(base, &[&Paragraph, &Bold, &TransformCase], "doc").unwrap();
    let strong = schema.mark_type("strong").unwrap().clone();
    let h = schema.text("h", vec![]).unwrap();
    let ello = schema
        .text("ello", vec![strong.create(Default::default())])
        .unwrap();
    let rest = schema.text(" world", vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![h, ello, rest], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    let mut s = EditorState::new(doc, schema);

    let mut t = s.tr();
    t.set_selection(Selection::Text { anchor: 1, head: 6 });
    s = s.apply(t);

    let s = run(s, &to_uppercase());
    assert_eq!(s.doc().text_content(), "HELLO world");
    // The strong mark must still cover "ELLO" — visible in the HTML.
    let html = s.doc().to_html();
    assert!(
        html.contains("<strong>ELLO</strong>"),
        "marks must be preserved across case transform: {html}"
    );
}

#[test]
fn case_commands_caret_only_is_a_noop() {
    let s = make_state("hello");
    let mut t = s.tr();
    t.set_selection(Selection::caret(2));
    let s = s.apply(t);
    assert!(!to_uppercase()(&s, None));
    assert!(!to_lowercase()(&s, None));
}
