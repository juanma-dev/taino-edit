//! Phase 6 Unit B: the block extensions (`Paragraph`, `Heading`).

use serde_json::json;
use taino_edit_core::{EditorState, KeyPress, NodeSpec, SchemaBuilder, Selection, Transaction};
use taino_edit_extensions::{build_keymap_with, build_schema_with, Extension, Heading, Paragraph};

fn base_builder() -> SchemaBuilder {
    // The base only declares the universal primitives. Paragraph/heading
    // extensions add the block types.
    SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
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

#[test]
fn paragraph_contributes_node_and_binding() {
    assert_eq!(Paragraph.name(), "paragraph");
    let adds = Paragraph.schema_additions();
    assert_eq!(adds.nodes.len(), 1);
    assert_eq!(adds.nodes[0].0, "paragraph");
    assert!(adds.marks.is_empty());

    let schema = build_schema_with(base_builder(), &[&Paragraph], "doc").unwrap();
    assert!(schema.node_type("paragraph").is_some());

    let bindings = Paragraph.keymap_entries(&schema);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "Mod-Alt-0");
}

#[test]
fn heading_contributes_node_with_level_attr() {
    assert_eq!(Heading.name(), "heading");
    let schema = build_schema_with(base_builder(), &[&Paragraph, &Heading], "doc").unwrap();
    let heading = schema.node_type("heading").expect("heading registered");
    assert!(
        heading.spec().attrs.contains_key("level"),
        "heading must declare a level attr"
    );
    let bindings = Heading.keymap_entries(&schema);
    assert_eq!(bindings.len(), 3, "three bindings (h1/h2/h3)");
    let keys: Vec<&str> = bindings.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["Mod-Alt-1", "Mod-Alt-2", "Mod-Alt-3"]);
}

#[test]
fn mod_alt_2_via_built_keymap_turns_paragraph_into_h2() {
    let schema = build_schema_with(base_builder(), &[&Paragraph, &Heading], "doc").unwrap();

    // Start with a single paragraph containing "Hi".
    let txt = schema.text("Hi", vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    let base = EditorState::new(doc, schema.clone());

    // Caret inside the paragraph (between H and i).
    let mut t = base.tr();
    t.set_selection(Selection::caret(2));
    let st = base.apply(t);

    let keymap = build_keymap_with(&[&Paragraph, &Heading], &schema, /*mac=*/ false);

    let mut next = None;
    {
        let mut dispatch = |tx: Transaction| {
            next = Some(st.apply(tx));
        };
        // Mod-Alt-2 = Ctrl+Alt+2 on non-mac.
        let handled = keymap.handle(&st, &KeyPress::key("2").ctrl().alt(), Some(&mut dispatch));
        assert!(handled, "Mod-Alt-2 must be bound");
    }
    let s2 = next.expect("dispatch called");
    assert_eq!(s2.doc().child(0).node_type().name(), "heading");
    assert_eq!(s2.doc().child(0).attrs().get("level"), Some(&json!(2)));
    assert_eq!(s2.doc().text_content(), "Hi", "text content is preserved");
}

#[test]
fn mod_alt_0_via_built_keymap_turns_heading_back_into_paragraph() {
    let schema = build_schema_with(base_builder(), &[&Paragraph, &Heading], "doc").unwrap();

    let mut hattrs = std::collections::BTreeMap::new();
    hattrs.insert("level".into(), json!(2));
    let txt = schema.text("Hi", vec![]).unwrap();
    let h = schema.node("heading", hattrs, vec![txt], vec![]).unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![h], vec![])
        .unwrap();
    let base = EditorState::new(doc, schema.clone());

    let mut t = base.tr();
    t.set_selection(Selection::caret(2));
    let st = base.apply(t);

    let keymap = build_keymap_with(&[&Paragraph, &Heading], &schema, false);
    let mut next = None;
    {
        let mut dispatch = |tx: Transaction| {
            next = Some(st.apply(tx));
        };
        let handled = keymap.handle(&st, &KeyPress::key("0").ctrl().alt(), Some(&mut dispatch));
        assert!(handled);
    }
    let s2 = next.unwrap();
    assert_eq!(s2.doc().child(0).node_type().name(), "paragraph");
    assert_eq!(s2.doc().text_content(), "Hi");
}
