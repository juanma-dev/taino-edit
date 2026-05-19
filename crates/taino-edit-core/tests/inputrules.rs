//! Phase 3 (input rules): regex-triggered text replacement, block retype
//! and block wrapping.

use std::collections::HashMap;

use serde_json::json;
use taino_edit_core::{
    text_replace_rule, textblock_type_rule, wrapping_rule, AttrSpec, Attrs, Captures, EditorState,
    InputRules, NodeSpec, Schema, SchemaBuilder, Selection,
};

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
            "blockquote",
            NodeSpec {
                content: Some("block+".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "heading",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                attrs: {
                    let mut m = HashMap::new();
                    m.insert(
                        "level".to_string(),
                        AttrSpec {
                            default: Some(json!(1)),
                        },
                    );
                    m
                },
                ..Default::default()
            },
        )
        .node(
            "paragraph",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
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

fn state_with(s: &Schema, text: &str, caret: usize) -> EditorState {
    let p = s
        .node(
            "paragraph",
            Default::default(),
            vec![s.text(text, vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    let d = s.node("doc", Default::default(), vec![p], vec![]).unwrap();
    let st = EditorState::new(d, s.clone());
    let mut t = st.tr();
    t.set_selection(Selection::caret(caret));
    st.apply(t)
}

fn level_from(caps: &Captures<'_>) -> Attrs {
    let lvl = caps.get(1).map(|m| m.as_str().len()).unwrap_or(1);
    let mut a = Attrs::new();
    a.insert("level".into(), json!(lvl));
    a
}

#[test]
fn text_replacement_rule() {
    let s = schema();
    let rules = InputRules::new(vec![text_replace_rule(r"\(c\)", "©").unwrap()]);
    // "a(c)" with caret at the end (pos 5).
    let st = state_with(&s, "a(c)", 5);
    let tx = rules.apply(&st).expect("rule matched");
    let next = st.apply(tx);
    assert_eq!(next.doc().text_content(), "a©");
}

#[test]
fn heading_input_rule() {
    let s = schema();
    let rules = InputRules::new(vec![textblock_type_rule(
        r"^(#{1,3})\s$",
        "heading",
        level_from,
    )
    .unwrap()]);
    // "## " at the start of a paragraph, caret after it (pos 4).
    let st = state_with(&s, "## ", 4);
    let tx = rules.apply(&st).expect("rule matched");
    let next = st.apply(tx);
    assert_eq!(next.doc().child(0).node_type().name(), "heading");
    assert_eq!(next.doc().child(0).attrs().get("level"), Some(&json!(2)));
    assert_eq!(next.doc().text_content(), "");
}

#[test]
fn blockquote_wrapping_rule() {
    let s = schema();
    let rules = InputRules::new(vec![
        wrapping_rule(r"^>\s$", "blockquote", Attrs::new()).unwrap()
    ]);
    // "> " at the start of a paragraph, caret after it (pos 3).
    let st = state_with(&s, "> ", 3);
    let tx = rules.apply(&st).expect("rule matched");
    let next = st.apply(tx);
    assert_eq!(next.doc().child(0).node_type().name(), "blockquote");
    assert_eq!(next.doc().child(0).child(0).node_type().name(), "paragraph");
}

#[test]
fn no_match_returns_none() {
    let s = schema();
    let rules = InputRules::new(vec![text_replace_rule(r"\(c\)", "©").unwrap()]);
    let st = state_with(&s, "hello", 6);
    assert!(rules.apply(&st).is_none());
    assert_eq!(rules.len(), 1);
}
