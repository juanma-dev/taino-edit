//! Phase 2: ReplaceAroundStep — wrap a block in a new parent and unwrap it
//! again via the inverse (the structural step behind wrap/lift).

use taino_edit_core::{Fragment, NodeSpec, ReplaceAroundStep, Schema, SchemaBuilder, Slice, Step};

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

#[test]
fn wraps_and_unwraps_a_paragraph() {
    let s = schema();
    let p = s
        .node(
            "paragraph",
            Default::default(),
            vec![s.text("Hi", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    let d = s
        .node("doc", Default::default(), vec![p.clone()], vec![])
        .unwrap();

    // Wrap the single paragraph (block range 0..4) in a blockquote.
    let wrapper = s
        .create_node("blockquote", Default::default(), vec![], vec![])
        .unwrap();
    let step = ReplaceAroundStep::new(
        0,
        4,
        0,
        4,
        Slice::new(Fragment::from_node(wrapper), 0, 0),
        1,
    );

    let wrapped = step.apply(&d, &s).unwrap();
    let expected = {
        let bq = s
            .node("blockquote", Default::default(), vec![p], vec![])
            .unwrap();
        s.node("doc", Default::default(), vec![bq], vec![]).unwrap()
    };
    assert_eq!(wrapped, expected, "paragraph is now inside a blockquote");

    // The inverse unwraps it back to the original document.
    let undo = step.invert(&d).unwrap();
    assert_eq!(undo.apply(&wrapped, &s).unwrap(), d, "unwrap restores doc");
}

#[test]
fn replace_around_get_map_and_json() {
    let s = schema();
    let wrapper = s
        .create_node("blockquote", Default::default(), vec![], vec![])
        .unwrap();
    let step = ReplaceAroundStep::new(
        0,
        4,
        0,
        4,
        Slice::new(Fragment::from_node(wrapper), 0, 0),
        1,
    );
    // Wrapping adds the blockquote's open+close tokens: end shifts by +2.
    assert_eq!(step.get_map().map(4, 1), 6);
    assert_eq!(step.to_json()["stepType"], "replaceAround");
}
