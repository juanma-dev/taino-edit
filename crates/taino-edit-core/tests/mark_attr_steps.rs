//! Phase 2: AddMark/RemoveMark/AttrStep — apply, the apply∘invert identity,
//! and JSON round-trip.

use std::collections::HashMap;

use serde_json::json;
use taino_edit_core::{
    step_from_json, AddMarkStep, AttrSpec, AttrStep, MarkSpec, Node, NodeSpec, RemoveMarkStep,
    Schema, SchemaBuilder, Step,
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

fn doc_para(s: &Schema, t: &str) -> Node {
    let p = s
        .node(
            "paragraph",
            Default::default(),
            vec![s.text(t, vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    s.node("doc", Default::default(), vec![p], vec![]).unwrap()
}

#[test]
fn add_mark_then_invert_is_identity() {
    let s = schema();
    let d = doc_para(&s, "Hello");
    let strong = s.mark_type("strong").unwrap().create(Default::default());

    let step = AddMarkStep::new(1, 6, strong.clone());
    let bolded = step.apply(&d, &s).unwrap();
    assert_eq!(
        bolded.child(0).child(0).marks(),
        std::slice::from_ref(&strong),
        "the whole word becomes strong"
    );

    let undo = step.invert(&d).unwrap();
    assert_eq!(undo.apply(&bolded, &s).unwrap(), d, "undo removes the mark");
}

#[test]
fn remove_mark_then_invert_is_identity() {
    let s = schema();
    let strong = s.mark_type("strong").unwrap().create(Default::default());
    let p = s
        .node(
            "paragraph",
            Default::default(),
            vec![s.text("Hello", vec![strong.clone()]).unwrap()],
            vec![],
        )
        .unwrap();
    let d = s.node("doc", Default::default(), vec![p], vec![]).unwrap();

    let step = RemoveMarkStep::new(1, 6, strong);
    let plain = step.apply(&d, &s).unwrap();
    assert!(plain.child(0).child(0).marks().is_empty());

    let undo = step.invert(&d).unwrap();
    assert_eq!(undo.apply(&plain, &s).unwrap(), d);
}

#[test]
fn attr_step_sets_and_inverts() {
    let s = schema();
    let h = s
        .node(
            "heading",
            Default::default(),
            vec![s.text("Hi", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    let d = s.node("doc", Default::default(), vec![h], vec![]).unwrap();
    assert_eq!(d.child(0).attrs().get("level"), Some(&json!(1)));

    let step = AttrStep::new(0, "level", json!(2));
    let updated = step.apply(&d, &s).unwrap();
    assert_eq!(updated.child(0).attrs().get("level"), Some(&json!(2)));

    let undo = step.invert(&d).unwrap();
    assert_eq!(undo.apply(&updated, &s).unwrap(), d, "level restored to 1");
}

#[test]
fn steps_json_round_trip() {
    let s = schema();
    let d = doc_para(&s, "Hello");
    let strong = s.mark_type("strong").unwrap().create(Default::default());

    let add = AddMarkStep::new(1, 6, strong);
    let v = add.to_json();
    assert_eq!(v["stepType"], "addMark");
    let restored = step_from_json(&s, &v).unwrap();
    assert_eq!(restored.apply(&d, &s).unwrap(), add.apply(&d, &s).unwrap());

    let attr = AttrStep::new(0, "level", json!(3));
    let restored_attr = step_from_json(&s, &attr.to_json()).unwrap();
    assert_eq!(restored_attr.to_json(), attr.to_json());
}
