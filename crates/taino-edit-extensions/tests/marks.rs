//! Phase 6 Unit A: the mark extensions (`Bold`, `Italic`) plug into the
//! schema and keymap pipeline.

use taino_edit_core::{
    EditorState, KeyPress, Node, NodeSpec, Schema, SchemaBuilder, Selection, Transaction,
};
use taino_edit_extensions::{build_keymap_with, build_schema_with, Bold, Extension, Italic};

/// A minimal base schema declaring the universal `doc`/`paragraph`/`text`
/// primitives — extensions are added on top.
fn base_builder() -> SchemaBuilder {
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
}

fn schema_with(exts: &[&dyn Extension]) -> Schema {
    build_schema_with(base_builder(), exts, "doc").unwrap()
}

fn doc_with(s: &Schema, text: &str) -> Node {
    let txt = s.text(text, vec![]).unwrap();
    let p = s
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    s.node("doc", Default::default(), vec![p], vec![]).unwrap()
}

#[test]
fn bold_contributes_strong_and_binding() {
    assert_eq!(Bold.name(), "bold");
    let adds = Bold.schema_additions();
    assert_eq!(adds.marks.len(), 1);
    assert_eq!(adds.marks[0].0, "strong");
    assert!(adds.nodes.is_empty());

    let schema = schema_with(&[&Bold]);
    assert!(schema.mark_type("strong").is_some());

    let bindings = Bold.keymap_entries(&schema);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "Mod-b");
}

#[test]
fn italic_contributes_em_and_binding() {
    assert_eq!(Italic.name(), "italic");
    let schema = schema_with(&[&Italic]);
    assert!(schema.mark_type("em").is_some());
    let bindings = Italic.keymap_entries(&schema);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "Mod-i");
}

#[test]
fn build_schema_with_aggregates_multiple_extensions() {
    let schema = schema_with(&[&Bold, &Italic]);
    assert!(schema.mark_type("strong").is_some());
    assert!(schema.mark_type("em").is_some());
}

#[test]
fn mod_b_via_built_keymap_toggles_strong_on_selection() {
    let schema = schema_with(&[&Bold, &Italic]);
    let doc = doc_with(&schema, "Hello");
    let base = EditorState::new(doc, schema.clone());

    // Selection over "Hello".
    let mut t = base.tr();
    t.set_selection(Selection::Text { anchor: 1, head: 6 });
    let st = base.apply(t);

    let keymap = build_keymap_with(&[&Bold, &Italic], &schema, /*mac=*/ false);
    assert!(keymap.len() >= 2 /* base + at least Mod-b/Mod-i */);

    let mut next = None;
    {
        let mut dispatch = |tx: Transaction| {
            next = Some(st.apply(tx));
        };
        let handled = keymap.handle(&st, &KeyPress::key("b").ctrl(), Some(&mut dispatch));
        assert!(handled, "Mod-b must be bound");
    }
    let bolded = next.expect("dispatch called");
    let marks = bolded.doc().child(0).child(0).marks();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].mark_type().name(), "strong");
}

#[test]
fn extensions_without_their_mark_in_the_schema_emit_no_bindings() {
    // Bold expects "strong" to exist; if the schema doesn't have it, the
    // extension just contributes nothing rather than panicking.
    let schema = build_schema_with(base_builder(), &[], "doc").unwrap();
    assert!(Bold.keymap_entries(&schema).is_empty());
}
