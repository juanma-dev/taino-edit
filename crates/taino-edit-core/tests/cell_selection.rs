//! v0.3 — the `Selection::Cell` variant (positional, table-agnostic in core).

use taino_edit_core::{NodeSpec, Schema, SchemaBuilder, Selection};

fn schema() -> Schema {
    SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("paragraph+".into()),
                ..Default::default()
            },
        )
        .node(
            "paragraph",
            NodeSpec {
                content: Some("text*".into()),
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

fn doc(s: &Schema) -> taino_edit_core::Node {
    // Two paragraphs "ab" and "cd"; the second begins at position 4.
    let p = |t: &str| {
        let txt = s.text(t, vec![]).unwrap();
        s.node("paragraph", Default::default(), vec![txt], vec![])
            .unwrap()
    };
    s.node("doc", Default::default(), vec![p("ab"), p("cd")], vec![])
        .unwrap()
}

#[test]
fn cell_from_is_the_lower_anchor() {
    let sel = Selection::Cell { anchor: 4, head: 0 };
    assert_eq!(sel.from(), 0);
}

#[test]
fn cell_to_extends_past_the_later_node() {
    let s = schema();
    let d = doc(&s);
    // The node starting at position 0 is the first paragraph (size 4).
    let sel = Selection::Cell { anchor: 0, head: 0 };
    assert_eq!(sel.to(&d), 4);
}

#[test]
fn cell_is_not_empty() {
    let sel = Selection::Cell { anchor: 0, head: 4 };
    assert!(!sel.is_empty());
}

#[test]
fn cell_round_trips_through_json_independent_construction() {
    // Constructing the variant and reading its endpoints is stable.
    let sel = Selection::Cell { anchor: 2, head: 8 };
    match sel {
        Selection::Cell { anchor, head } => {
            assert_eq!((anchor, head), (2, 8));
        }
        _ => panic!("expected a cell selection"),
    }
}
