//! Phase 2: the tree-replace algorithm — insert, delete, block joins,
//! nested descent, the cut↔replace identity, and schema/​depth rejection.

use taino_edit_core::{Fragment, Node, NodeSpec, ReplaceError, Schema, SchemaBuilder, Slice};

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

fn para(s: &Schema, t: &str) -> Node {
    let txt = s.text(t, vec![]).unwrap();
    s.node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap()
}

fn doc(s: &Schema, ps: Vec<Node>) -> Node {
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

fn text_slice(s: &Schema, t: &str) -> Slice {
    Slice::new(Fragment::from_node(s.text(t, vec![]).unwrap()), 0, 0)
}

#[test]
fn inserts_text_into_a_paragraph() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "Hello")]);
    // p occupies 0..7; "Hello" content is 1..6, so pos 6 is end-of-text.
    let out = d.replace(6, 6, &text_slice(&s, " world"), &s).unwrap();
    assert_eq!(out, doc(&s, vec![para(&s, "Hello world")]));
}

#[test]
fn deletes_a_range() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "Hello world")]);
    let out = d.replace(6, 12, &Slice::empty(), &s).unwrap();
    assert_eq!(out, doc(&s, vec![para(&s, "Hello")]));
}

#[test]
fn joins_two_paragraphs() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "foo"), para(&s, "bar")]);
    // End of the first paragraph's text (4) to start of the second's (6).
    let out = d.replace(4, 6, &Slice::empty(), &s).unwrap();
    assert_eq!(out, doc(&s, vec![para(&s, "foobar")]));
}

#[test]
fn descends_into_nested_blocks() {
    let s = schema();
    let inner = s
        .node(
            "blockquote",
            Default::default(),
            vec![para(&s, "a"), para(&s, "b")],
            vec![],
        )
        .unwrap();
    let d = doc(&s, vec![inner]);
    // "a" sits at absolute 2..3; insert at 3 (end of "a").
    let out = d.replace(3, 3, &text_slice(&s, "X"), &s).unwrap();

    let expected_inner = s
        .node(
            "blockquote",
            Default::default(),
            vec![para(&s, "aX"), para(&s, "b")],
            vec![],
        )
        .unwrap();
    assert_eq!(out, doc(&s, vec![expected_inner]));
}

#[test]
fn cut_then_reinsert_is_identity() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "Hello"), para(&s, "World")]);
    // Replacing any range with exactly its own slice must be a no-op.
    for (from, to) in [(1, 1), (2, 5), (3, 9), (1, 13), (0, d.content().size())] {
        let sl = d.slice(from, to).unwrap();
        let out = d.replace(from, to, &sl, &s).unwrap();
        assert_eq!(out, d, "replace({from},{to}, own slice) must be identity");
    }
}

#[test]
fn slice_records_open_depths() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "Hello"), para(&s, "World")]);
    // From inside the first paragraph to inside the second.
    let sl = d.slice(3, 11).unwrap();
    assert_eq!(sl.open_start(), 1);
    assert_eq!(sl.open_end(), 1);
    assert_eq!(sl.content().child_count(), 2);
}

#[test]
fn rejects_schema_violating_replacement() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "Hello")]);
    // A block paragraph spliced into inline text violates `inline*`.
    let block_slice = Slice::new(Fragment::from_node(para(&s, "x")), 0, 0);
    assert!(matches!(
        d.replace(3, 3, &block_slice, &s),
        Err(ReplaceError::InvalidContent { .. })
    ));
}

#[test]
fn rejects_inconsistent_open_depth() {
    let s = schema();
    let d = doc(&s, vec![para(&s, "Hello")]);
    let too_open = Slice::new(Fragment::from_node(s.text("x", vec![]).unwrap()), 5, 5);
    assert!(matches!(
        d.replace(3, 3, &too_open, &s),
        Err(ReplaceError::OpenTooDeep)
    ));
}
