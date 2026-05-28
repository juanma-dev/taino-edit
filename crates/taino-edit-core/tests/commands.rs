//! Phase 3 (commands, part 1): selection + mark commands, applicability
//! probing, and command chaining.

use taino_edit_core::{
    caret_left, caret_right, chain, delete_selection, remove_mark, select_all, set_mark,
    toggle_mark, Command, EditorState, MarkSpec, Node, NodeSpec, Schema, SchemaBuilder, Selection,
    Transaction,
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

fn list_schema() -> Schema {
    SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "bullet_list",
            NodeSpec {
                content: Some("list_item+".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "list_item",
            NodeSpec {
                content: Some("paragraph+".into()),
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

/// A bullet list with one item per string.
fn list_doc(s: &Schema, items: &[&str]) -> Node {
    let lis: Vec<Node> = items
        .iter()
        .map(|t| {
            let p = s
                .node("paragraph", Default::default(), vec![s.text(t, vec![]).unwrap()], vec![])
                .unwrap();
            s.node("list_item", Default::default(), vec![p], vec![])
                .unwrap()
        })
        .collect();
    let ul = s.node("bullet_list", Default::default(), lis, vec![]).unwrap();
    s.node("doc", Default::default(), vec![ul], vec![]).unwrap()
}

#[test]
fn caret_right_at_doc_end_does_not_overshoot_into_a_boundary() {
    // <ul><li><p>Rust</p></li></ul>; caret at the end of the text (pos 7).
    // The next raw position (8) sits inside the <li> after </p> — a
    // non-textblock boundary. Caret motion must refuse to land there.
    let s = list_schema();
    let st = with_selection(
        EditorState::new(list_doc(&s, &["Rust"]), s.clone()),
        Selection::caret(7),
    );
    let cmd: Command = Box::new(caret_right);
    assert!(
        !cmd(&st, None),
        "ArrowRight at the last text position must be a no-op, not an overshoot"
    );
}

#[test]
fn caret_right_crosses_into_the_next_list_item_text() {
    // <ul><li><p>ab</p></li><li><p>cd</p></li></ul>. End of "ab" is pos 5;
    // start of "cd" is pos 9. The boundary positions 6/7/8 must be skipped.
    let s = list_schema();
    let st = with_selection(
        EditorState::new(list_doc(&s, &["ab", "cd"]), s.clone()),
        Selection::caret(5),
    );
    let next = run(&(Box::new(caret_right) as Command), &st).unwrap();
    assert_eq!(next.selection(), Selection::caret(9));
}

#[test]
fn caret_left_crosses_back_into_the_previous_list_item_text() {
    let s = list_schema();
    let st = with_selection(
        EditorState::new(list_doc(&s, &["ab", "cd"]), s.clone()),
        Selection::caret(9), // start of "cd"
    );
    let next = run(&(Box::new(caret_left) as Command), &st).unwrap();
    assert_eq!(next.selection(), Selection::caret(5)); // end of "ab"
}

#[test]
fn join_backward_joins_paragraph_into_list() {
    let base = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "bullet_list",
            NodeSpec {
                content: Some("list_item+".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "list_item",
            NodeSpec {
                content: Some("paragraph+".into()),
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
        .unwrap();

    let s = base;
    let p1 = s.node("paragraph", Default::default(), vec![s.text("Hello", vec![]).unwrap()], vec![]).unwrap();
    let li = s.node("list_item", Default::default(), vec![p1], vec![]).unwrap();
    let ul = s.node("bullet_list", Default::default(), vec![li], vec![]).unwrap();
    let p2 = s.node("paragraph", Default::default(), vec![s.text("World", vec![]).unwrap()], vec![]).unwrap();
    let doc = s.node("doc", Default::default(), vec![ul, p2], vec![]).unwrap();

    let st = with_selection(EditorState::new(doc, s.clone()), Selection::caret(12));
    let cmd: Command = Box::new(taino_edit_core::join_backward);
    assert!(cmd(&st, None));
    let next = run(&cmd, &st).unwrap();

    let final_doc = next.doc();
    assert_eq!(final_doc.child_count(), 1);
    let final_ul = final_doc.child(0);
    assert_eq!(final_ul.node_type().name(), "bullet_list");
    let final_li = final_ul.child(0);
    assert_eq!(final_li.node_type().name(), "list_item");
    let final_p = final_li.child(0);
    assert_eq!(final_p.node_type().name(), "paragraph");
    assert_eq!(final_p.text_content(), "HelloWorld");
    assert_eq!(next.selection(), Selection::caret(8));
}

#[test]
fn join_backward_joins_paragraph_into_list_with_text() {
    let base = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "bullet_list",
            NodeSpec {
                content: Some("list_item+".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "list_item",
            NodeSpec {
                content: Some("paragraph+".into()),
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
        .unwrap();

    let s = base;
    let p1 = s.node("paragraph", Default::default(), vec![s.text("HelloWorld test", vec![]).unwrap()], vec![]).unwrap();
    let li = s.node("list_item", Default::default(), vec![p1], vec![]).unwrap();
    let ul = s.node("bullet_list", Default::default(), vec![li], vec![]).unwrap();
    let p2 = s.node("paragraph", Default::default(), vec![s.text("abc", vec![]).unwrap()], vec![]).unwrap();
    let doc = s.node("doc", Default::default(), vec![ul, p2], vec![]).unwrap();

    let st = with_selection(EditorState::new(doc, s.clone()), Selection::caret(22));
    let cmd: Command = Box::new(taino_edit_core::join_backward);
    assert!(cmd(&st, None));
    let next = run(&cmd, &st).expect("join_backward should produce a new state");

    let final_doc = next.doc();
    assert_eq!(final_doc.child_count(), 1);
    let final_ul = final_doc.child(0);
    assert_eq!(final_ul.node_type().name(), "bullet_list");
    let final_li = final_ul.child(0);
    assert_eq!(final_li.node_type().name(), "list_item");
    let final_p = final_li.child(0);
    assert_eq!(final_p.node_type().name(), "paragraph");
    assert_eq!(final_p.text_content(), "HelloWorld testabc");
    assert_eq!(next.selection(), Selection::caret(18));
}

#[test]
fn test_split_at_depth_2() {
    let base = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "bullet_list",
            NodeSpec {
                content: Some("list_item+".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "list_item",
            NodeSpec {
                content: Some("paragraph+".into()),
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
        .unwrap();

    let s = base;
    let p1 = s.node("paragraph", Default::default(), vec![s.text("Test List", vec![]).unwrap()], vec![]).unwrap();
    let li = s.node("list_item", Default::default(), vec![p1], vec![]).unwrap();
    let ul = s.node("bullet_list", Default::default(), vec![li], vec![]).unwrap();
    let doc = s.node("doc", Default::default(), vec![ul], vec![]).unwrap();

    let st = EditorState::new(doc, s.clone());
    let mut tx = st.tr();
    let res = tx.transform().split_at_depth(12, 2, &s);
    if let Err(e) = &res {
        println!("split_at_depth failed with: {:?}", e.0);
    }
    assert!(res.is_ok());
}

#[test]
fn test_split_heading_in_list() {
    let base = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "bullet_list",
            NodeSpec {
                content: Some("list_item+".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "list_item",
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
        .unwrap();

    let s = base;
    let p1 = s.node("heading", Default::default(), vec![s.text("Test List", vec![]).unwrap()], vec![]).unwrap();
    let li = s.node("list_item", Default::default(), vec![p1], vec![]).unwrap();
    let ul = s.node("bullet_list", Default::default(), vec![li], vec![]).unwrap();
    let doc = s.node("doc", Default::default(), vec![ul], vec![]).unwrap();

    let st = EditorState::new(doc, s.clone());
    let mut tx = st.tr();
    let res = tx.transform().split_at_depth(12, 2, &s);
    if let Err(e) = &res {
        println!("split_heading_in_list failed with: {:?}", e.0);
    }
    assert!(res.is_ok());
}
