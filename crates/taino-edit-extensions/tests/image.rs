//! Phase 6 Unit D: the `Image` inline atom extension.

use serde_json::json;
use taino_edit_core::{Command, EditorState, NodeSpec, SchemaBuilder, Selection};
use taino_edit_extensions::{build_schema_with, insert_image, Extension, Image, Paragraph};

fn run(state: EditorState, cmd: &Command) -> EditorState {
    let mut next = None;
    {
        let mut d = |tx| next = Some(state.apply(tx));
        cmd(&state, Some(&mut d));
    }
    next.unwrap_or(state)
}

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
    let schema = build_schema_with(base, &[&Paragraph, &Image], "doc").unwrap();
    let txt = schema.text("ab", vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    EditorState::new(doc, schema)
}

#[test]
fn image_contributes_inline_leaf() {
    assert_eq!(Image.name(), "image");
    let adds = Image.schema_additions();
    assert_eq!(adds.nodes.len(), 1);
    let (name, spec) = &adds.nodes[0];
    assert_eq!(name, "image");
    assert!(spec.inline);
    assert!(spec.atom);
    assert!(spec.attrs.contains_key("src"));
    assert!(spec.attrs.contains_key("alt"));
}

#[test]
fn insert_image_replaces_selection_with_an_image_node() {
    let s = make_state();
    let mut t = s.tr();
    t.set_selection(Selection::caret(2)); // caret between "a" and "b"
    let s = s.apply(t);

    let cmd = insert_image("https://example.com/cat.png", Some("a cat".into()));
    assert!(cmd(&s, None), "always-applies command");
    let s = run(s, &cmd);

    let html = s.doc().to_html();
    assert!(
        html.contains("<img src=\"https://example.com/cat.png\" alt=\"a cat\"/>"),
        "expected image in serialized HTML, got: {html}"
    );

    // JSON shows the image node sitting between the two text runs.
    let j = s.doc().to_json();
    let kids = j["content"][0]["content"].as_array().unwrap();
    assert_eq!(kids.len(), 3, "split text + image + tail");
    assert_eq!(kids[1]["type"], json!("image"));
    assert_eq!(kids[1]["attrs"]["src"], json!("https://example.com/cat.png"));
}

#[test]
fn insert_image_at_caret_grows_doc_size_by_one() {
    let s = make_state();
    let before = s.doc().content().size();
    let mut t = s.tr();
    t.set_selection(Selection::caret(1));
    let s = s.apply(t);
    let s = run(s, &insert_image("x.png", None));
    let after = s.doc().content().size();
    assert_eq!(after, before + 1, "image atom is one position wide");
}
