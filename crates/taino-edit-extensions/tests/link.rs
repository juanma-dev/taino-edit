//! Phase 6 Unit D: the `Link` mark extension.

use serde_json::json;
use taino_edit_core::{Command, EditorState, NodeSpec, SchemaBuilder, Selection};
use taino_edit_extensions::{
    build_schema_with, remove_link, set_link, Bold, Extension, Link, Paragraph,
};

fn run(state: EditorState, cmd: &Command) -> EditorState {
    let mut out = state.clone();
    let mut next = None;
    let mut d = |tx| next = Some(state.apply(tx));
    cmd(&state, Some(&mut d));
    if let Some(n) = next {
        out = n;
    }
    out
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
    let schema = build_schema_with(base, &[&Paragraph, &Bold, &Link], "doc").unwrap();
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
fn link_contributes_mark_with_href_attr() {
    assert_eq!(Link.name(), "link");
    let adds = Link.schema_additions();
    assert!(adds.nodes.is_empty());
    assert_eq!(adds.marks.len(), 1);
    let (name, spec) = &adds.marks[0];
    assert_eq!(name, "link");
    assert!(spec.attrs.contains_key("href"));
    assert!(!spec.inclusive, "link must NOT extend on typing at its edge");
}

#[test]
fn set_link_wraps_selection_in_link_mark() {
    let s = make_state();
    // Select "hello" (positions 1..6 inside the paragraph).
    let mut t = s.tr();
    t.set_selection(Selection::Text { anchor: 1, head: 6 });
    let s = s.apply(t);

    let cmd = set_link("https://example.com", None);
    let mut next = None;
    {
        let mut d = |tx| next = Some(s.apply(tx));
        let applies = cmd(&s, Some(&mut d));
        assert!(applies);
    }
    let s2 = next.expect("dispatch fired");
    // The first text node should carry a `link` mark with the matching href.
    let html = s2.doc().to_html();
    assert!(
        html.contains("<a href=\"https://example.com\">hello</a>"),
        "expected anchor in serialized HTML, got: {html}"
    );

    // And the JSON round-trips the attrs.
    let j = s2.doc().to_json();
    let para = &j["content"][0]["content"];
    let first = &para[0];
    assert!(first["marks"].as_array().unwrap().iter().any(|m| {
        m["type"] == "link" && m["attrs"]["href"] == json!("https://example.com")
    }));
}

#[test]
fn set_link_replaces_an_existing_link_on_the_same_range() {
    let s = make_state();
    let mut t = s.tr();
    t.set_selection(Selection::Text { anchor: 1, head: 6 });
    let s = s.apply(t);

    let state = run(s, &set_link("https://one.example", None));
    let state = run(state, &set_link("https://two.example", None));
    let html = state.doc().to_html();
    assert!(html.contains("https://two.example"));
    assert!(
        !html.contains("https://one.example"),
        "old href must be stripped, got: {html}"
    );
}

#[test]
fn remove_link_strips_link_marks() {
    let s = make_state();
    let mut t = s.tr();
    t.set_selection(Selection::Text { anchor: 1, head: 6 });
    let state = s.apply(t);
    let state = run(state, &set_link("https://example.com", None));
    let cmd = remove_link();
    assert!(cmd(&state, None), "remove_link must apply when a link covers the selection");
    let state = run(state, &cmd);
    let html = state.doc().to_html();
    assert!(
        !html.contains("<a "),
        "no anchor should remain after remove_link: {html}"
    );
}

#[test]
fn set_link_caret_only_is_a_noop() {
    let s = make_state();
    let mut t = s.tr();
    t.set_selection(Selection::caret(3));
    let s = s.apply(t);
    let cmd = set_link("https://example.com", None);
    let applies = cmd(&s, None);
    assert!(!applies, "with no range selected, set_link should not apply");
}
