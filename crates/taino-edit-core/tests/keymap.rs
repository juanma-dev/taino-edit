//! Phase 3 (keymap): base bindings, Mod platform handling, and that every
//! base command is reachable through a key.

use taino_edit_core::{
    base_keymap, EditorState, KeyPress, Keymap, Node, NodeSpec, Schema, SchemaBuilder, Selection,
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
        .top_node("doc")
        .build()
        .unwrap()
}

fn doc(s: &Schema, blocks: Vec<&str>) -> Node {
    let ps: Vec<Node> = blocks
        .iter()
        .map(|t| {
            s.node(
                "paragraph",
                Default::default(),
                vec![s.text(t, vec![]).unwrap()],
                vec![],
            )
            .unwrap()
        })
        .collect();
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

fn at(st: EditorState, pos: usize) -> EditorState {
    let mut t = st.tr();
    t.set_selection(Selection::caret(pos));
    st.apply(t)
}

fn press(km: &Keymap, st: &EditorState, k: KeyPress) -> Option<EditorState> {
    let mut out = None;
    {
        let mut d = |tx: Transaction| out = Some(st.apply(tx));
        km.handle(st, &k, Some(&mut d));
    }
    out
}

#[test]
fn mod_is_ctrl_off_mac_and_cmd_on_mac() {
    let s = schema();
    let st = EditorState::new(doc(&s, vec!["Hello"]), s.clone());

    let pc = base_keymap(false);
    assert!(pc.handle(&st, &KeyPress::key("a").ctrl(), None));
    assert!(!pc.handle(&st, &KeyPress::key("a").meta(), None));

    let mac = base_keymap(true);
    assert!(mac.handle(&st, &KeyPress::key("a").meta(), None));
    assert!(!mac.handle(&st, &KeyPress::key("a").ctrl(), None));

    let next = press(&pc, &st, KeyPress::key("a").ctrl()).unwrap();
    assert_eq!(next.selection(), Selection::All);
}

#[test]
fn enter_splits_the_block() {
    let s = schema();
    let st = at(EditorState::new(doc(&s, vec!["abcd"]), s.clone()), 3);
    let km = base_keymap(false);
    let out = press(&km, &st, KeyPress::key("Enter")).unwrap();
    assert_eq!(out.doc().child_count(), 2);
    assert_eq!(out.doc().child(1).text_content(), "cd");
}

#[test]
fn backspace_chain_covers_char_and_block_join() {
    let s = schema();
    let km = base_keymap(false);

    // Caret at end of "abc" (pos 4) → Backspace deletes the last char.
    let st = at(EditorState::new(doc(&s, vec!["abc"]), s.clone()), 4);
    let out = press(&km, &st, KeyPress::key("Backspace")).unwrap();
    assert_eq!(out.doc().text_content(), "ab");

    // Caret at start of 2nd block → join with the first.
    let two = at(EditorState::new(doc(&s, vec!["ab", "cd"]), s.clone()), 5);
    let joined = press(&km, &two, KeyPress::key("Backspace")).unwrap();
    assert_eq!(joined.doc().child_count(), 1);
    assert_eq!(joined.doc().text_content(), "abcd");
}

#[test]
fn delete_chain_pulls_next_block() {
    let s = schema();
    let km = base_keymap(false);
    // Caret at end of 1st block → Delete joins the next block up.
    let st = at(EditorState::new(doc(&s, vec!["ab", "cd"]), s.clone()), 3);
    let out = press(&km, &st, KeyPress::key("Delete")).unwrap();
    assert_eq!(out.doc().child_count(), 1);
    assert_eq!(out.doc().text_content(), "abcd");
}

#[test]
fn caret_motion_keys() {
    let s = schema();
    let km = base_keymap(false);
    let st = at(EditorState::new(doc(&s, vec!["abcd"]), s.clone()), 3);

    assert_eq!(
        press(&km, &st, KeyPress::key("ArrowLeft"))
            .unwrap()
            .selection(),
        Selection::caret(2)
    );
    assert_eq!(
        press(&km, &st, KeyPress::key("ArrowRight"))
            .unwrap()
            .selection(),
        Selection::caret(4)
    );
    assert_eq!(
        press(&km, &st, KeyPress::key("Home")).unwrap().selection(),
        Selection::caret(1)
    );
    assert_eq!(
        press(&km, &st, KeyPress::key("End")).unwrap().selection(),
        Selection::caret(5)
    );
}

#[test]
fn shift_is_implicit_for_symbol_keys() {
    // A binding `"Mod->"` should still match a press whose `key=">"`
    // carries shift=true (which browsers always send for symbol keys on
    // US layouts). Letter-key bindings must not be affected.
    use taino_edit_core::{select_all, Command};
    let mut km = base_keymap(false);
    let hit = std::rc::Rc::new(std::cell::Cell::new(false));
    let h = hit.clone();
    let cmd: Command = Box::new(move |_, _| {
        h.set(true);
        true
    });
    km.add("Mod->", cmd);

    let s = schema();
    let st = EditorState::new(doc(&s, vec!["x"]), s.clone());
    assert!(
        km.handle(&st, &KeyPress::key(">").ctrl().shift(), None),
        "Mod-> must match a Ctrl+Shift+> press"
    );
    assert!(hit.get(), "the bound command must have run");

    // Sanity: a lowercase letter binding is NOT promoted via shift-strip
    // (Mod-Shift-z must stay distinct from Mod-z).
    let mut km = base_keymap(false);
    km.add("Mod-z", Box::new(select_all));
    assert!(km.handle(&st, &KeyPress::key("z").ctrl(), None));
    assert!(!km.handle(&st, &KeyPress::key("Z").ctrl().shift(), None));
}

#[test]
fn unknown_key_is_unhandled() {
    let s = schema();
    let km = base_keymap(false);
    let st = EditorState::new(doc(&s, vec!["x"]), s.clone());
    assert!(!km.handle(&st, &KeyPress::key("F5"), None));
    assert!(km.len() >= 8);
}
