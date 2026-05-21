//! Phase 6 Unit I: the `Lists` extension.

use taino_edit_core::{Command, EditorState, KeyPress, NodeSpec, SchemaBuilder, Selection};
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, lift_list_item, wrap_in_bullet_list,
    wrap_in_ordered_list, Extension, Lists, Paragraph,
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
    let schema = build_schema_with(base, &[&Paragraph, &Lists], "doc").unwrap();
    let txt = schema.text(text, vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    let s = EditorState::new(doc, schema);
    let mut t = s.tr();
    t.set_selection(Selection::caret(1));
    s.apply(t)
}

#[test]
fn lists_registers_three_node_types_and_three_bindings() {
    let adds = Lists.schema_additions();
    let names: Vec<&str> = adds.nodes.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["list_item", "bullet_list", "ordered_list"]);

    let s = make_state("hi");
    let bindings = Lists.keymap_entries(s.schema());
    let keys: Vec<&str> = bindings.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["Mod-Shift-8", "Mod-Shift-7", "Shift-Tab"]);
}

#[test]
fn wrap_in_bullet_list_wraps_paragraph() {
    let s = make_state("hello");
    let s = run(s, &wrap_in_bullet_list());
    let html = s.doc().to_html();
    assert!(
        html.contains("<ul><li><p>hello</p></li></ul>"),
        "expected bullet list, got: {html}"
    );
}

#[test]
fn wrap_in_ordered_list_wraps_paragraph() {
    let s = make_state("hello");
    let s = run(s, &wrap_in_ordered_list());
    let html = s.doc().to_html();
    assert!(
        html.contains("<ol><li><p>hello</p></li></ol>"),
        "expected ordered list, got: {html}"
    );
}

#[test]
fn lift_list_item_unwraps_single_item_list() {
    let s = make_state("hello");
    let s = run(s, &wrap_in_bullet_list());
    // Place caret inside the paragraph (paragraph is at deep depth now).
    // After the wrap, the original text is at depth 4: doc>ul>li>p>text.
    // The paragraph's first char is at position 4.
    let mut t = s.tr();
    t.set_selection(Selection::caret(4));
    let s = s.apply(t);

    let s = run(s, &lift_list_item());
    let html = s.doc().to_html();
    assert!(
        html.contains("<p>hello</p>"),
        "after lift, paragraph should be back at top level: {html}"
    );
    assert!(
        !html.contains("<ul>") && !html.contains("<li>"),
        "list wrappers should be gone: {html}"
    );
}

#[test]
fn lists_via_built_keymap_mod_shift_8() {
    let s = make_state("hello");
    let keymap = build_keymap_with(&[&Paragraph, &Lists], s.schema(), false);
    let mut next = None;
    {
        let mut d = |tx| next = Some(s.apply(tx));
        let handled = keymap.handle(&s, &KeyPress::key("8").ctrl().shift(), Some(&mut d));
        assert!(handled, "Mod-Shift-8 must be bound (bullet list)");
    }
    let s2 = next.expect("dispatch fired");
    let html = s2.doc().to_html();
    assert!(html.contains("<ul>"), "expected ul wrapper, got: {html}");
}

#[test]
fn ordered_list_round_trips_through_html() {
    let s = make_state("hello");
    let s = run(s, &wrap_in_ordered_list());
    let html = s.doc().to_html();
    let parsed = s.schema().parse_html(&html).expect("parse");
    assert_eq!(parsed.child(0).node_type().name(), "ordered_list");
    assert_eq!(parsed.text_content(), "hello");
}
