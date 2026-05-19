//! Phase 3 (commands, part 2): block + join commands and split.

use std::collections::HashMap;

use serde_json::json;
use taino_edit_core::{
    join_backward, join_forward, lift, set_block_type, split_block, wrap_in, AttrSpec, Attrs,
    Command, EditorState, Node, NodeSpec, Schema, SchemaBuilder, Selection, Transaction,
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

fn para(s: &Schema, t: &str) -> Node {
    s.node(
        "paragraph",
        Default::default(),
        vec![s.text(t, vec![]).unwrap()],
        vec![],
    )
    .unwrap()
}

fn doc(s: &Schema, blocks: Vec<Node>) -> Node {
    s.node("doc", Default::default(), blocks, vec![]).unwrap()
}

fn at(st: EditorState, pos: usize) -> EditorState {
    let mut t = st.tr();
    t.set_selection(Selection::caret(pos));
    st.apply(t)
}

fn run(cmd: &Command, st: &EditorState) -> Option<EditorState> {
    let mut out = None;
    {
        let mut d = |tx: Transaction| out = Some(st.apply(tx));
        cmd(st, Some(&mut d));
    }
    out
}

#[test]
fn set_block_type_changes_paragraph_to_heading() {
    let s = schema();
    let st = at(
        EditorState::new(doc(&s, vec![para(&s, "Hi")]), s.clone()),
        2,
    );
    let mut attrs = Attrs::new();
    attrs.insert("level".into(), json!(2));
    let cmd = set_block_type("heading", attrs);
    assert!(cmd(&st, None));
    let out = run(&cmd, &st).unwrap();
    assert_eq!(out.doc().child(0).node_type().name(), "heading");
    assert_eq!(out.doc().child(0).attrs().get("level"), Some(&json!(2)));
    assert_eq!(out.doc().text_content(), "Hi");
}

#[test]
fn wrap_in_blockquote_then_lift_back() {
    let s = schema();
    let st = at(
        EditorState::new(doc(&s, vec![para(&s, "Hi")]), s.clone()),
        2,
    );

    let wrap = wrap_in("blockquote", Attrs::new());
    let wrapped = run(&wrap, &st).unwrap();
    assert_eq!(wrapped.doc().child(0).node_type().name(), "blockquote");
    assert_eq!(
        wrapped.doc().child(0).child(0).node_type().name(),
        "paragraph"
    );

    // Caret now sits inside doc>blockquote>paragraph (depth 2). Lift it out.
    let inside = at(wrapped, 3);
    let lift_cmd: Command = Box::new(lift);
    assert!(lift_cmd(&inside, None));
    let lifted = run(&lift_cmd, &inside).unwrap();
    assert_eq!(lifted.doc(), &doc(&s, vec![para(&s, "Hi")]));
}

#[test]
fn split_block_divides_a_paragraph() {
    let s = schema();
    // "abcd" → caret after "ab" at absolute pos 3.
    let st = at(
        EditorState::new(doc(&s, vec![para(&s, "abcd")]), s.clone()),
        3,
    );
    let cmd: Command = Box::new(split_block);
    assert!(cmd(&st, None));
    let out = run(&cmd, &st).unwrap();
    assert_eq!(out.doc().child_count(), 2);
    assert_eq!(out.doc().child(0).text_content(), "ab");
    assert_eq!(out.doc().child(1).text_content(), "cd");
    assert_eq!(out.selection(), Selection::caret(5));
}

#[test]
fn join_backward_merges_with_previous_block() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "ab"), para(&s, "cd")]);
    // Start of the 2nd paragraph's content is absolute pos 5.
    let st = at(EditorState::new(d, s.clone()), 5);
    let cmd: Command = Box::new(join_backward);
    assert!(cmd(&st, None));
    let out = run(&cmd, &st).unwrap();
    assert_eq!(out.doc(), &doc(&s, vec![para(&s, "abcd")]));
    assert_eq!(out.selection(), Selection::caret(3));
}

#[test]
fn join_forward_pulls_next_block_up() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "ab"), para(&s, "cd")]);
    // End of the 1st paragraph's content is absolute pos 3.
    let st = at(EditorState::new(d, s.clone()), 3);
    let cmd: Command = Box::new(join_forward);
    assert!(cmd(&st, None));
    let out = run(&cmd, &st).unwrap();
    assert_eq!(out.doc(), &doc(&s, vec![para(&s, "abcd")]));
}

#[test]
fn join_backward_not_applicable_for_first_block() {
    let s = schema();
    let st = at(
        EditorState::new(doc(&s, vec![para(&s, "ab")]), s.clone()),
        1,
    );
    assert!(!join_backward(&st, None), "no preceding sibling");
}
