//! Phase 2: ReplaceStep — apply, the apply∘invert identity (the heart of
//! undo), position map, and lossless JSON round-trip.

use taino_edit_core::{
    step_from_json, Fragment, Node, NodeSpec, ReplaceStep, Schema, SchemaBuilder, Slice, Step,
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
    let txt = s.text(t, vec![]).unwrap();
    s.node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap()
}

fn doc1(s: &Schema, t: &str) -> Node {
    s.node("doc", Default::default(), vec![para(s, t)], vec![])
        .unwrap()
}

fn text_slice(s: &Schema, t: &str) -> Slice {
    Slice::new(Fragment::from_node(s.text(t, vec![]).unwrap()), 0, 0)
}

#[test]
fn applies_and_maps() {
    let s = schema();
    let d = doc1(&s, "Hello");
    let step = ReplaceStep::new(6, 6, text_slice(&s, " world"));

    let out = step.apply(&d, &s).unwrap();
    assert_eq!(out, doc1(&s, "Hello world"));

    let m = step.get_map();
    assert_eq!(m.map(2, 1), 2, "before the insertion");
    assert_eq!(m.map(6, 1), 12, "after a 6-wide insertion");
}

#[test]
fn apply_then_invert_is_identity() {
    let s = schema();

    // Insertion.
    let d = doc1(&s, "Hello");
    let ins = ReplaceStep::new(6, 6, text_slice(&s, " world"));
    let after = ins.apply(&d, &s).unwrap();
    let undo = ins.invert(&d).unwrap();
    assert_eq!(undo.apply(&after, &s).unwrap(), d, "undo of an insert");

    // Deletion.
    let d2 = doc1(&s, "Hello world");
    let del = ReplaceStep::new(6, 12, Slice::empty());
    let after2 = del.apply(&d2, &s).unwrap();
    assert_eq!(after2, doc1(&s, "Hello"));
    let redo = del.invert(&d2).unwrap();
    assert_eq!(redo.apply(&after2, &s).unwrap(), d2, "undo of a delete");
}

#[test]
fn json_round_trips() {
    let s = schema();
    let d = doc1(&s, "Hello");
    let step = ReplaceStep::new(6, 6, text_slice(&s, " world"));

    let v = step.to_json();
    assert_eq!(v["stepType"], "replace");

    let restored = step_from_json(&s, &v).expect("re-parse");
    assert_eq!(
        restored.apply(&d, &s).unwrap(),
        step.apply(&d, &s).unwrap(),
        "a round-tripped step behaves identically"
    );
}

#[test]
fn empty_replace_is_a_no_op_step() {
    let s = schema();
    let d = doc1(&s, "abc");
    let step = ReplaceStep::new(2, 2, Slice::empty());
    assert_eq!(step.apply(&d, &s).unwrap(), d);
}
