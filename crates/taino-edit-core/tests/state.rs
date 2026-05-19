//! Phase 2 DoD: transforms accumulate steps + mapping, and undo/redo is
//! correct across every step type, bounded, groupable and skippable.

use std::collections::HashMap;

use serde_json::json;
use taino_edit_core::{
    AttrSpec, EditorState, Fragment, History, MarkSpec, Node, NodeSpec, Schema, SchemaBuilder,
    Selection, Slice, Transform,
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
        .mark("strong", MarkSpec::default())
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

fn doc(s: &Schema, t: &str) -> Node {
    s.node("doc", Default::default(), vec![para(s, t)], vec![])
        .unwrap()
}

fn text_slice(s: &Schema, t: &str) -> Slice {
    Slice::new(Fragment::from_node(s.text(t, vec![]).unwrap()), 0, 0)
}

#[test]
fn transform_accumulates_steps_and_mapping() {
    let s = schema();
    let mut tr = Transform::new(doc(&s, "Hello"));
    tr.insert(6, text_slice(&s, " world"), &s)
        .unwrap()
        .delete(1, 6, &s)
        .unwrap();
    assert_eq!(tr.steps().len(), 2);
    assert_eq!(tr.doc(), &doc(&s, " world"));
    // A position after both edits maps through the combined mapping.
    assert_eq!(tr.mapping().map(6, 1), 7);
}

#[test]
fn undo_redo_replace() {
    let s = schema();
    let st0 = EditorState::new(doc(&s, "Hello"), s.clone());
    let mut tx = st0.tr();
    tx.transform()
        .insert(6, text_slice(&s, " world"), &s)
        .unwrap();
    let st1 = st0.apply(tx);
    assert_eq!(st1.doc(), &doc(&s, "Hello world"));

    let undone = st1.undo().expect("undo");
    assert_eq!(undone.doc(), &doc(&s, "Hello"));
    let redone = undone.redo().expect("redo");
    assert_eq!(redone.doc(), &doc(&s, "Hello world"));
}

#[test]
fn undo_redo_across_all_step_types() {
    let s = schema();
    let strong = s.mark_type("strong").unwrap().create(Default::default());

    let h = s
        .node(
            "heading",
            Default::default(),
            vec![s.text("Title", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    let base = s
        .node("doc", Default::default(), vec![h, para(&s, "body")], vec![])
        .unwrap();
    let st = EditorState::new(base.clone(), s.clone());

    // replace, then addMark, then attr — three separate undo groups.
    let mut t1 = st.tr();
    t1.transform().delete(8, 9, &s).unwrap(); // drop a char in "Title"
    let s1 = st.apply(t1);

    let mut t2 = s1.tr();
    t2.transform().add_mark(1, 5, strong.clone(), &s).unwrap();
    let s2 = s1.apply(t2);

    let mut t3 = s2.tr();
    t3.transform()
        .set_node_attr(0, "level", json!(2), &s)
        .unwrap();
    let s3 = s2.apply(t3);

    assert_eq!(s3.doc().child(0).attrs().get("level"), Some(&json!(2)));

    // Undo all three, LIFO, back to the original document.
    let u1 = s3.undo().unwrap();
    assert_eq!(u1.doc().child(0).attrs().get("level"), Some(&json!(1)));
    let u2 = u1.undo().unwrap();
    assert!(u2.doc().child(0).child(0).marks().is_empty());
    let u3 = u2.undo().unwrap();
    assert_eq!(u3.doc(), &base, "fully undone == original");
    assert!(u3.undo().is_none(), "nothing left to undo");

    // Redo climbs back up to the final document.
    let r = u3.redo().unwrap().redo().unwrap().redo().unwrap();
    assert_eq!(r.doc(), s3.doc());
}

#[test]
fn history_is_bounded() {
    let s = schema();
    let mut st = EditorState::new(doc(&s, "x"), s.clone()).with_history(History::with_depth(2));
    for _ in 0..4 {
        let mut tx = st.tr();
        tx.transform().insert(2, text_slice(&s, "a"), &s).unwrap();
        st = st.apply(tx);
    }
    assert_eq!(st.history().undo_depth(), 2, "older groups dropped");
    let a = st.undo().unwrap();
    let b = a.undo().unwrap();
    assert!(b.undo().is_none());
}

#[test]
fn grouping_and_skipping_history() {
    let s = schema();
    let empty = s
        .node(
            "doc",
            Default::default(),
            vec![s
                .node("paragraph", Default::default(), vec![], vec![])
                .unwrap()],
            vec![],
        )
        .unwrap();
    let st = EditorState::new(empty.clone(), s.clone());

    // Two joined transactions collapse into one undo group.
    let mut t1 = st.tr();
    t1.transform().insert(1, text_slice(&s, "a"), &s).unwrap();
    let s1 = st.apply(t1);
    let mut t2 = s1.tr();
    t2.transform().insert(2, text_slice(&s, "b"), &s).unwrap();
    t2.join_history();
    let s2 = s1.apply(t2);
    assert_eq!(s2.doc(), &doc(&s, "ab"));
    assert_eq!(s2.history().undo_depth(), 1, "joined into one group");
    let undone = s2.undo().unwrap();
    assert_eq!(undone.doc(), &empty, "one undo reverts both");

    // no_history transactions don't record.
    let mut t3 = s2.tr();
    t3.transform().insert(3, text_slice(&s, "c"), &s).unwrap();
    t3.no_history();
    let s3 = s2.apply(t3);
    assert_eq!(s3.history().undo_depth(), 1, "unchanged by no_history tx");
}

#[test]
fn selection_maps_through_a_transaction() {
    let s = schema();
    let st = EditorState::new(doc(&s, "Hello"), s.clone());
    let st = {
        let mut t = st.tr();
        t.set_selection(Selection::caret(6));
        st.apply(t)
    };
    assert_eq!(st.selection(), Selection::caret(6));

    // Insert before the caret; it should ride forward by the inserted size.
    let mut t = st.tr();
    t.transform().insert(1, text_slice(&s, "XYZ"), &s).unwrap();
    let next = st.apply(t);
    assert_eq!(next.selection(), Selection::caret(9));
}
