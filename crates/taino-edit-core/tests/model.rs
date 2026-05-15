//! Phase 1 acceptance: schema validation, content matching, JSON round-trip
//! without loss, and position-resolution edge cases.

use std::collections::HashMap;

use serde_json::json;
use taino_edit_core::{
    AttrSpec, DocError, MarkSpec, Node, NodeSpec, ResolvedPos, Schema, SchemaBuilder, SchemaError,
};

fn attr(default: serde_json::Value) -> HashMap<String, AttrSpec> {
    let mut m = HashMap::new();
    m.insert(
        "level".to_string(),
        AttrSpec {
            default: Some(default),
        },
    );
    m
}

fn src_attr() -> HashMap<String, AttrSpec> {
    let mut m = HashMap::new();
    m.insert(
        "src".to_string(),
        AttrSpec {
            default: Some(json!("")),
        },
    );
    m
}

/// doc(block+) > {paragraph(inline*), heading(inline*, level), image leaf} ;
/// marks: strong, em.
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
            "heading",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                attrs: attr(json!(1)),
                ..Default::default()
            },
        )
        .node(
            "image",
            NodeSpec {
                group: Some("inline".into()),
                inline: true,
                attrs: src_attr(),
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
        .mark("em", MarkSpec::default())
        .top_node("doc")
        .build()
        .expect("schema builds")
}

fn sample_doc(s: &Schema) -> Node {
    let title = s.text("Title", vec![]).unwrap();
    let heading = s
        .node("heading", Default::default(), vec![title], vec![])
        .unwrap();

    let strong = s.mark_type("strong").unwrap().clone();
    let hello = s.text("Hello ", vec![]).unwrap();
    let world = s
        .text("world", vec![strong.create(Default::default())])
        .unwrap();
    let img = s.node("image", Default::default(), vec![], vec![]).unwrap();
    let para = s
        .node(
            "paragraph",
            Default::default(),
            vec![hello, world, img],
            vec![],
        )
        .unwrap();

    s.node("doc", Default::default(), vec![heading, para], vec![])
        .unwrap()
}

#[test]
fn schema_build_errors() {
    let unknown_ref = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("para+".into()),
                ..Default::default()
            },
        )
        .node("text", NodeSpec::default())
        .build();
    assert!(matches!(
        unknown_ref,
        Err(SchemaError::UnknownContentRef { .. })
    ));

    let dup = SchemaBuilder::new()
        .node("doc", NodeSpec::default())
        .node("doc", NodeSpec::default())
        .build();
    assert!(matches!(dup, Err(SchemaError::DuplicateType(_))));

    let bad_top = SchemaBuilder::new()
        .node("para", NodeSpec::default())
        .top_node("doc")
        .build();
    assert!(matches!(bad_top, Err(SchemaError::UnknownTopNode(_))));

    assert!(matches!(
        SchemaBuilder::new().build(),
        Err(SchemaError::Empty)
    ));
}

#[test]
fn content_expression_validation() {
    let s = schema();

    // doc requires at least one block; empty content is rejected.
    let empty_doc = s.node("doc", Default::default(), vec![], vec![]);
    assert!(matches!(empty_doc, Err(DocError::InvalidContent { .. })));

    // A block is fine.
    let p = s
        .node("paragraph", Default::default(), vec![], vec![])
        .unwrap();
    assert!(s
        .node("doc", Default::default(), vec![p.clone()], vec![])
        .is_ok());

    // doc may not directly contain inline content.
    let t = s.text("x", vec![]).unwrap();
    assert!(matches!(
        s.node("doc", Default::default(), vec![t], vec![]),
        Err(DocError::InvalidContent { .. })
    ));

    // Leaf/atom flags.
    assert!(s.node_type("text").unwrap().is_leaf());
    assert!(s.node_type("image").unwrap().is_leaf());
    assert!(!s.node_type("doc").unwrap().is_leaf());
    assert!(s.node_type("image").unwrap().is_inline());
    assert!(s.node_type("paragraph").unwrap().is_block());
}

#[test]
fn node_sizes() {
    let s = schema();
    let doc = sample_doc(&s);
    // "Title"=5 → heading 5+2=7 ; "Hello "=6 + "world"=5 + image=1 = 12 →
    // paragraph 12+2=14 ; doc content = 7+14 = 21.
    assert_eq!(doc.child(0).node_size(), 7);
    assert_eq!(doc.child(1).node_size(), 14);
    assert_eq!(doc.content().size(), 21);
    assert_eq!(doc.text_content(), "TitleHello world");
}

#[test]
fn json_round_trips_without_loss() {
    let s = schema();
    let doc = sample_doc(&s);

    let value = doc.to_json();
    let back = s.node_from_json(&value).expect("re-parse");
    assert_eq!(doc, back, "document must survive a JSON round-trip");

    // And through a string.
    let text = serde_json::to_string(&value).unwrap();
    let back2 = s.parse_json_str(&text).unwrap();
    assert_eq!(doc, back2);

    // Marks and attrs are preserved.
    let para = back.child(1);
    assert_eq!(para.child(1).text(), Some("world"));
    assert_eq!(para.child(1).marks()[0].mark_type().name(), "strong");
    assert_eq!(
        back.child(0).attrs().get("level"),
        Some(&json!(1)),
        "default attrs are filled and preserved"
    );
}

#[test]
fn json_rejects_invalid_documents() {
    let s = schema();

    let unknown = json!({ "type": "blink", "content": [] });
    assert!(matches!(
        s.node_from_json(&unknown),
        Err(DocError::UnknownNodeType(_))
    ));

    // paragraph directly inside a paragraph violates `inline*`.
    let bad = json!({
        "type": "doc",
        "content": [ { "type": "paragraph", "content": [
            { "type": "paragraph" }
        ] } ]
    });
    assert!(matches!(
        s.node_from_json(&bad),
        Err(DocError::InvalidContent { .. })
    ));

    let malformed = json!([1, 2, 3]);
    assert!(matches!(
        s.node_from_json(&malformed),
        Err(DocError::MalformedJson(_))
    ));
}

#[test]
fn resolve_position_edge_cases() {
    let s = schema();
    let doc = sample_doc(&s);

    // Start of the document.
    let r0 = ResolvedPos::resolve(&doc, 0).unwrap();
    assert_eq!(r0.depth(), 0);
    assert_eq!(r0.parent().node_type().name(), "doc");
    assert_eq!(r0.index(0), 0);

    // Boundary between the heading and the paragraph (doc-level).
    let r7 = ResolvedPos::resolve(&doc, 7).unwrap();
    assert_eq!(r7.depth(), 0);
    assert_eq!(r7.index(0), 1);
    assert_eq!(r7.start(0), 0);

    // Inside the heading's text: position 2 ⇒ between 'T' and 'i'.
    let r2 = ResolvedPos::resolve(&doc, 2).unwrap();
    assert_eq!(r2.depth(), 1);
    assert_eq!(r2.parent().node_type().name(), "heading");
    assert_eq!(r2.text_offset(), 1);
    assert_eq!(r2.start(1), 1);

    // Just inside the paragraph (before its first child).
    let r8 = ResolvedPos::resolve(&doc, 8).unwrap();
    assert_eq!(r8.depth(), 1);
    assert_eq!(r8.parent().node_type().name(), "paragraph");
    assert_eq!(r8.before(1), 7, "position directly before the paragraph");

    // End of the document.
    let end = doc.content().size();
    let r_end = ResolvedPos::resolve(&doc, end).unwrap();
    assert_eq!(r_end.depth(), 0);
    assert_eq!(r_end.index(0), doc.child_count());

    // Out of range.
    assert!(matches!(
        ResolvedPos::resolve(&doc, end + 1),
        Err(DocError::PositionOutOfRange { .. })
    ));
}

#[test]
fn mark_set_operations() {
    let s = schema();
    let strong = s.mark_type("strong").unwrap().create(Default::default());
    let em = s.mark_type("em").unwrap().create(Default::default());

    let set = strong.add_to_set(&[]);
    let set = em.add_to_set(&set);
    assert_eq!(set.len(), 2);
    assert!(strong.is_in_set(&set));

    // Re-adding the same type does not duplicate it.
    let set2 = strong.add_to_set(&set);
    assert_eq!(set2.len(), 2);

    let removed = strong.remove_from_set(&set);
    assert_eq!(removed.len(), 1);
    assert!(!strong.is_in_set(&removed));
    assert!(taino_edit_core::same_mark_set(&removed, &[em]));
}
