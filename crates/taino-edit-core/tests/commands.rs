//! Phase 3 (commands, part 1): selection + mark commands, applicability
//! probing, and command chaining.

use taino_edit_core::{
    chain, delete_selection, remove_mark, select_all, set_mark, toggle_mark, Command, EditorState,
    MarkSpec, Node, NodeSpec, Schema, SchemaBuilder, Selection, Transaction,
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
        .mark("strong", MarkSpec::default())
        .top_node("doc")
        .build()
        .unwrap()
}

fn doc(s: &Schema, t: &str) -> Node {
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

fn with_selection(st: EditorState, sel: Selection) -> EditorState {
    let mut t = st.tr();
    t.set_selection(sel);
    st.apply(t)
}

fn run(cmd: &Command, st: &EditorState) -> Option<EditorState> {
    let mut out = None;
    {
        let mut d = |tx: Transaction| out = Some(st.apply(tx));
        cmd(st, Some(&mut d));
    }
    out
}

#[test]
fn select_all_selects_document() {
    let s = schema();
    let st = EditorState::new(doc(&s, "Hello"), s.clone());
    let cmd: Command = Box::new(select_all);
    assert!(cmd(&st, None), "always applicable");
    let next = run(&cmd, &st).unwrap();
    assert_eq!(next.selection(), Selection::All);
}

#[test]
fn delete_selection_only_when_non_empty() {
    let s = schema();
    let st = EditorState::new(doc(&s, "Hello"), s.clone());
    let del: Command = Box::new(delete_selection);

    // Caret → not applicable.
    assert!(!del(&st, None));

    // Range 1..4 ("Hel") → deletes it.
    let ranged = with_selection(st, Selection::Text { anchor: 1, head: 4 });
    assert!(del(&ranged, None));
    let after = run(&del, &ranged).unwrap();
    assert_eq!(after.doc(), &doc(&s, "lo"));
    assert_eq!(after.selection(), Selection::caret(1));
}

#[test]
fn toggle_set_remove_mark() {
    let s = schema();
    let strong = s.mark_type("strong").unwrap().clone();
    let base = EditorState::new(doc(&s, "Hello"), s.clone());
    let sel = Selection::Text { anchor: 1, head: 6 };
    let st = with_selection(base, sel);

    let toggle = toggle_mark(strong.clone());
    assert!(toggle(&st, None));

    // First toggle adds the mark to the whole word.
    let bold = run(&toggle, &st).unwrap();
    assert_eq!(
        bold.doc().child(0).child(0).marks().len(),
        1,
        "strong applied"
    );

    // Toggling again removes it.
    let plain = run(&toggle, &with_selection(bold.clone(), sel)).unwrap();
    assert!(plain.doc().child(0).child(0).marks().is_empty());

    // Explicit set/remove.
    let set = set_mark(strong.clone());
    let removed = remove_mark(strong);
    let s2 = run(&set, &st).unwrap();
    assert_eq!(s2.doc().child(0).child(0).marks().len(), 1);
    let s3 = run(&removed, &with_selection(s2, sel)).unwrap();
    assert!(s3.doc().child(0).child(0).marks().is_empty());
}

#[test]
fn toggle_mark_not_applicable_on_caret() {
    let s = schema();
    let strong = s.mark_type("strong").unwrap().clone();
    let st = EditorState::new(doc(&s, "Hello"), s.clone()); // caret at 0
    assert!(!toggle_mark(strong)(&st, None));
}

#[test]
fn chain_runs_first_applicable() {
    let s = schema();
    let st = EditorState::new(doc(&s, "Hello"), s.clone()); // caret
                                                            // delete_selection is not applicable on a caret, so the chain falls
                                                            // through to select_all.
    let c = chain(vec![Box::new(delete_selection), Box::new(select_all)]);
    assert!(c(&st, None));
    let next = run(&c, &st).unwrap();
    assert_eq!(next.selection(), Selection::All);
}
