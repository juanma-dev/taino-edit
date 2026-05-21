//! Phase 6 Unit E: the `Align` block-attribute extension.

use serde_json::json;
use taino_edit_core::{Command, EditorState, KeyPress, NodeSpec, SchemaBuilder, Selection};
use taino_edit_extensions::{
    align_center, align_left, align_right, build_keymap_with, build_schema_with, Align, Extension,
    Heading, Paragraph,
};

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
    let schema =
        build_schema_with(base, &[&Paragraph, &Heading, &Align], "doc").expect("schema builds");
    let txt = schema.text("hi", vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    let mut s = EditorState::new(doc, schema);
    let mut t = s.tr();
    t.set_selection(Selection::caret(1));
    s = s.apply(t);
    s
}

#[test]
fn paragraph_declares_text_align_attr() {
    let adds = Paragraph.schema_additions();
    let (_, spec) = &adds.nodes[0];
    assert!(
        spec.attrs.contains_key("text_align"),
        "paragraph must declare text_align so Align can set it"
    );
}

#[test]
fn heading_declares_text_align_attr() {
    let adds = Heading.schema_additions();
    let (_, spec) = &adds.nodes[0];
    assert!(
        spec.attrs.contains_key("text_align"),
        "heading must declare text_align so Align can set it"
    );
}

#[test]
fn align_extension_binds_four_keys() {
    let s = make_state();
    let bindings = Align.keymap_entries(s.schema());
    let keys: Vec<&str> = bindings.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        vec!["Mod-Shift-l", "Mod-Shift-e", "Mod-Shift-r", "Mod-Shift-j"]
    );
}

#[test]
fn align_center_sets_text_align_attr_and_style() {
    let s = make_state();
    let s = run(s, &align_center());
    let j = s.doc().to_json();
    assert_eq!(
        j["content"][0]["attrs"]["text_align"],
        json!("center"),
        "text_align attr must be written by align_center"
    );
    let html = s.doc().to_html();
    assert!(
        html.contains("style=\"text-align: center\""),
        "centered paragraph must emit text-align style, got: {html}"
    );
}

#[test]
fn align_left_then_default_round_trip() {
    let s = make_state();
    let s = run(s, &align_right());
    assert_eq!(
        s.doc().to_json()["content"][0]["attrs"]["text_align"],
        json!("right")
    );
    let s = run(s, &align_left());
    assert_eq!(
        s.doc().to_json()["content"][0]["attrs"]["text_align"],
        json!("left")
    );
    let html = s.doc().to_html();
    assert!(
        html.contains("text-align: left"),
        "left-align must emit the style: {html}"
    );
}

#[test]
fn align_via_built_keymap() {
    let s = make_state();
    let keymap = build_keymap_with(&[&Paragraph, &Heading, &Align], s.schema(), false);
    let mut next = None;
    {
        let mut d = |tx| next = Some(s.apply(tx));
        let handled = keymap.handle(&s, &KeyPress::key("e").ctrl().shift(), Some(&mut d));
        assert!(handled, "Mod-Shift-e must be bound (align_center)");
    }
    let s2 = next.expect("dispatch fired");
    assert_eq!(
        s2.doc().to_json()["content"][0]["attrs"]["text_align"],
        json!("center")
    );
}
