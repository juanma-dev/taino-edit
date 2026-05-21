//! Phase 6 Unit C: the History extension wires undo/redo into the keymap
//! pipeline through a HistoryIntent transaction.

use taino_edit_core::{
    EditorState, Fragment, KeyPress, NodeSpec, SchemaBuilder, Slice, Transaction,
};
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, redo_command, undo_command, Extension, History, Paragraph,
};

fn schema_and_doc() -> (taino_edit_core::Schema, EditorState) {
    let base = SchemaBuilder::new()
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
        );
    let schema = build_schema_with(base, &[&Paragraph], "doc").unwrap();
    let txt = schema.text("a", vec![]).unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![p], vec![])
        .unwrap();
    let s0 = EditorState::new(doc, schema.clone());
    (schema, s0)
}

fn type_b(state: &EditorState, schema: &taino_edit_core::Schema) -> EditorState {
    let mut t = state.tr();
    let txt = schema.text("b", vec![]).unwrap();
    let slice = Slice::new(Fragment::from_node(txt), 0, 0);
    t.transform().insert(2, slice, schema).unwrap();
    state.apply(t)
}

#[test]
fn undo_command_walks_history_via_intent_transaction() {
    let (schema, s0) = schema_and_doc();
    let s1 = type_b(&s0, &schema);
    assert_eq!(s1.doc().text_content(), "ab");
    assert_eq!(s1.history().undo_depth(), 1);

    let undo = undo_command();
    assert!(undo(&s1, None), "applicable while there's history");

    let mut next = None;
    {
        let mut dispatch = |tx: Transaction| next = Some(s1.apply(tx));
        let handled = undo(&s1, Some(&mut dispatch));
        assert!(handled);
    }
    let s2 = next.expect("dispatched");
    assert_eq!(s2.doc(), s0.doc(), "undo restores the prior doc");
    assert_eq!(s2.history().undo_depth(), 0);
    assert_eq!(s2.history().redo_depth(), 1);
}

#[test]
fn undo_not_applicable_without_history() {
    let (_, s0) = schema_and_doc();
    let undo = undo_command();
    assert!(!undo(&s0, None));
}

#[test]
fn redo_after_undo_recovers_the_change() {
    let (schema, s0) = schema_and_doc();
    let s1 = type_b(&s0, &schema);

    let undo = undo_command();
    let redo = redo_command();

    let mut after_undo = None;
    {
        let mut d = |tx| after_undo = Some(s1.apply(tx));
        undo(&s1, Some(&mut d));
    }
    let undone = after_undo.unwrap();
    assert!(redo(&undone, None));

    let mut after_redo = None;
    {
        let mut d = |tx| after_redo = Some(undone.apply(tx));
        redo(&undone, Some(&mut d));
    }
    let redone = after_redo.unwrap();
    assert_eq!(redone.doc(), s1.doc());
}

#[test]
fn mod_z_via_built_keymap_undoes() {
    let (schema, s0) = schema_and_doc();
    let s1 = type_b(&s0, &schema);

    let keymap = build_keymap_with(&[&Paragraph, &History], &schema, /*mac=*/ false);
    assert!(
        keymap.handle(&s1, &KeyPress::key("z").ctrl(), None),
        "Mod-z must be bound"
    );

    let mut next = None;
    {
        let mut d = |tx| next = Some(s1.apply(tx));
        keymap.handle(&s1, &KeyPress::key("z").ctrl(), Some(&mut d));
    }
    let s2 = next.unwrap();
    assert_eq!(s2.doc(), s0.doc());
}

#[test]
fn mod_shift_z_via_built_keymap_redoes() {
    let (schema, s0) = schema_and_doc();
    let s1 = type_b(&s0, &schema);

    // Undo via the same keymap, then redo.
    let keymap = build_keymap_with(&[&Paragraph, &History], &schema, false);
    let mut after_undo = None;
    {
        let mut d = |tx| after_undo = Some(s1.apply(tx));
        keymap.handle(&s1, &KeyPress::key("z").ctrl(), Some(&mut d));
    }
    let undone = after_undo.unwrap();
    assert_eq!(undone.doc(), s0.doc());

    let mut after_redo = None;
    {
        let mut d = |tx| after_redo = Some(undone.apply(tx));
        keymap.handle(&undone, &KeyPress::key("z").ctrl().shift(), Some(&mut d));
    }
    let redone = after_redo.unwrap();
    assert_eq!(redone.doc(), s1.doc());
}

#[test]
fn history_extension_name_and_bindings() {
    let (schema, _) = schema_and_doc();
    assert_eq!(History.name(), "history");
    let bindings = History.keymap_entries(&schema);
    let keys: Vec<&str> = bindings.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["Mod-z", "Mod-Shift-z"]);
}
