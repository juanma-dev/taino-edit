//! Phase 6 Unit I + Phase v0.2.1: the `Lists` extension.

use taino_edit_core::{Command, EditorState, KeyPress, NodeSpec, Schema, SchemaBuilder, Selection};
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, lift_list_item, sink_list_item, smart_enter_in_list,
    split_list_item, wrap_in_bullet_list, wrap_in_ordered_list, Extension, Lists, Paragraph,
};

fn run(state: EditorState, cmd: &Command) -> EditorState {
    let mut next = None;
    {
        let mut d = |tx| next = Some(state.apply(tx));
        cmd(&state, Some(&mut d));
    }
    next.unwrap_or(state)
}

fn schema() -> Schema {
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
    build_schema_with(base, &[&Paragraph, &Lists], "doc").unwrap()
}

fn paragraph(s: &Schema, text: &str) -> taino_edit_core::Node {
    let txt = s.text(text, vec![]).unwrap();
    s.node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap()
}

fn state_with_paragraph(text: &str) -> EditorState {
    let s = schema();
    let p = paragraph(&s, text);
    let doc = s.node("doc", Default::default(), vec![p], vec![]).unwrap();
    let st = EditorState::new(doc, s);
    let mut t = st.tr();
    t.set_selection(Selection::caret(1));
    st.apply(t)
}

#[test]
fn lists_registers_three_nodes_and_five_bindings() {
    let adds = Lists.schema_additions();
    let names: Vec<&str> = adds.nodes.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["list_item", "bullet_list", "ordered_list"]);

    let s = state_with_paragraph("hi");
    let bindings = Lists.keymap_entries(s.schema());
    let keys: Vec<&str> = bindings.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        vec!["Mod-Shift-8", "Mod-Shift-7", "Tab", "Shift-Tab", "Enter"]
    );
}

#[test]
fn wrap_in_bullet_list_wraps_paragraph() {
    let s = state_with_paragraph("hello");
    let s = run(s, &wrap_in_bullet_list());
    let html = s.doc().to_html();
    assert!(
        html.contains("<ul><li><p>hello</p></li></ul>"),
        "expected bullet list, got: {html}"
    );
}

#[test]
fn wrap_in_ordered_list_wraps_paragraph() {
    let s = state_with_paragraph("hello");
    let s = run(s, &wrap_in_ordered_list());
    let html = s.doc().to_html();
    assert!(
        html.contains("<ol><li><p>hello</p></li></ol>"),
        "expected ordered list, got: {html}"
    );
}

#[test]
fn lift_list_item_unwraps_single_item_list() {
    let s = state_with_paragraph("hello");
    let s = run(s, &wrap_in_bullet_list());
    let mut t = s.tr();
    t.set_selection(Selection::caret(4));
    let s = s.apply(t);

    let s = run(s, &lift_list_item());
    let html = s.doc().to_html();
    assert!(html.contains("<p>hello</p>"));
    assert!(!html.contains("<ul>") && !html.contains("<li>"));
}

#[test]
fn lift_list_item_from_multi_item_list_keeps_siblings() {
    let s = schema();
    let li = |t: &str| {
        s.create_node(
            "list_item",
            Default::default(),
            vec![paragraph(&s, t)],
            vec![],
        )
        .unwrap()
    };
    let list = s
        .create_node(
            "bullet_list",
            Default::default(),
            vec![li("a"), li("b"), li("c")],
            vec![],
        )
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![list], vec![])
        .unwrap();
    let st = EditorState::new(doc, s.clone());
    // Caret inside "b" (second list_item). Doc structure:
    // [doc][ul][li]a[/li][li]b[/li][li]c[/li][/ul][/doc]
    // li "a" starts at 1, ends at 6 (node_size = li(p(text))) = 2+(2+1)+ ... let me compute:
    //   p(text="a") node_size = text_len + 2 = 3
    //   li(p) node_size = p_size + 2 = 5
    //   ul(li,li,li) inner size = 15, but the doc-level boundary positions need a resolve.
    // Easiest: place caret with select_all-like math by resolving forward.
    let mut t = st.tr();
    // Caret near the start of li "b"'s text. Position 7 lands inside li 2's paragraph.
    t.set_selection(Selection::caret(7));
    let st = st.apply(t);

    let st = run(st, &lift_list_item());
    let html = st.doc().to_html();
    // After lifting the middle item, "b" sits between two single-item lists.
    assert!(
        html.contains("<ul><li><p>a</p></li></ul><p>b</p><ul><li><p>c</p></li></ul>"),
        "lift didn't preserve siblings: {html}"
    );
}

#[test]
fn smart_enter_splits_list_item_into_two() {
    let s = schema();
    let li = s
        .create_node(
            "list_item",
            Default::default(),
            vec![paragraph(&s, "hello")],
            vec![],
        )
        .unwrap();
    let list = s
        .create_node("bullet_list", Default::default(), vec![li], vec![])
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![list], vec![])
        .unwrap();
    let st = EditorState::new(doc, s.clone());
    // Position layout: 0=before ul, 1=inside ul (before li), 2=inside li
    // (before p), 3=inside p (text offset 0), so 6 = text offset 3 = between
    // "hel" and "lo".
    let mut t = st.tr();
    t.set_selection(Selection::caret(6));
    let st = st.apply(t);

    let st = run(st, &smart_enter_in_list());
    let html = st.doc().to_html();
    assert!(
        html.contains("<ul><li><p>hel</p></li><li><p>lo</p></li></ul>"),
        "smart enter should split into two list_items: {html}"
    );
}

#[test]
fn smart_enter_on_empty_list_item_exits_the_list() {
    let s = schema();
    let empty_li = s
        .create_node(
            "list_item",
            Default::default(),
            vec![paragraph(&s, "")],
            vec![],
        )
        .unwrap();
    let list = s
        .create_node("bullet_list", Default::default(), vec![empty_li], vec![])
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![list], vec![])
        .unwrap();
    let st = EditorState::new(doc, s.clone());
    // Caret inside the empty paragraph: pos 3 ([doc][ul][li][p|]).
    let mut t = st.tr();
    t.set_selection(Selection::caret(3));
    let st = st.apply(t);

    let st = run(st, &smart_enter_in_list());
    let html = st.doc().to_html();
    assert!(
        !html.contains("<ul>") && !html.contains("<li>"),
        "empty bullet + Enter should exit the list: {html}"
    );
    assert!(html.contains("<p></p>"));
}

#[test]
fn split_list_item_is_alias_of_smart_enter() {
    // Both return the same closure semantics; we don't compare pointers,
    // just behaviour.
    let s = schema();
    let li = s
        .create_node(
            "list_item",
            Default::default(),
            vec![paragraph(&s, "ab")],
            vec![],
        )
        .unwrap();
    let list = s
        .create_node("bullet_list", Default::default(), vec![li], vec![])
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![list], vec![])
        .unwrap();
    let st = EditorState::new(doc, s.clone());
    let mut t = st.tr();
    t.set_selection(Selection::caret(4)); // between 'a' and 'b'
    let st = st.apply(t);

    let st = run(st, &split_list_item());
    let html = st.doc().to_html();
    assert!(
        html.contains("<li><p>a</p></li><li><p>b</p></li>"),
        "split should produce two list_items with one char each: {html}"
    );
}

#[test]
fn sink_list_item_nests_under_previous_sibling() {
    let s = schema();
    let li = |t: &str| {
        s.create_node(
            "list_item",
            Default::default(),
            vec![paragraph(&s, t)],
            vec![],
        )
        .unwrap()
    };
    let list = s
        .create_node(
            "bullet_list",
            Default::default(),
            vec![li("a"), li("b")],
            vec![],
        )
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![list], vec![])
        .unwrap();
    let st = EditorState::new(doc, s.clone());
    // Caret inside "b".
    let mut t = st.tr();
    t.set_selection(Selection::caret(7));
    let st = st.apply(t);

    let st = run(st, &sink_list_item());
    let html = st.doc().to_html();
    assert!(
        html.contains("<ul><li><p>a</p><ul><li><p>b</p></li></ul></li></ul>"),
        "sink should nest 'b' under 'a': {html}"
    );
}

#[test]
fn sink_list_item_first_item_is_a_noop() {
    let s = state_with_paragraph("hello");
    let s = run(s, &wrap_in_bullet_list());
    let mut t = s.tr();
    t.set_selection(Selection::caret(4));
    let s = s.apply(t);

    let cmd = sink_list_item();
    assert!(
        !cmd(&s, None),
        "single-item or first-item sink should report not applicable"
    );
}

#[test]
fn lists_via_built_keymap_mod_shift_8_and_tab() {
    let s = state_with_paragraph("hello");
    let keymap = build_keymap_with(&[&Paragraph, &Lists], s.schema(), false);

    // Wrap in bullet list via Mod-Shift-8.
    let mut next = None;
    {
        let mut d = |tx| next = Some(s.apply(tx));
        let handled = keymap.handle(&s, &KeyPress::key("8").ctrl().shift(), Some(&mut d));
        assert!(handled);
    }
    let s = next.expect("wrap dispatched");
    assert!(s.doc().to_html().contains("<ul>"));

    // Tab on the single (first) item is a no-op; the keymap reports false.
    let handled = keymap.handle(&s, &KeyPress::key("Tab"), None);
    assert!(!handled, "Tab on first/only item must not apply");
}

#[test]
fn ordered_list_round_trips_through_html() {
    let s = state_with_paragraph("hello");
    let s = run(s, &wrap_in_ordered_list());
    let html = s.doc().to_html();
    let parsed = s.schema().parse_html(&html).expect("parse");
    assert_eq!(parsed.child(0).node_type().name(), "ordered_list");
    assert_eq!(parsed.text_content(), "hello");
}
