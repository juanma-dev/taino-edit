//! v0.5 hardening: structural editing inside lists driven through the same
//! pipeline the adapters use (command → state → `view.update` +
//! `set_selection` → read-back). Reproduces the Enter / empty-item / typing
//! sequences that misbehaved in the demo.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::KeyPress;
use taino_edit_core::{Command, EditorState, Node, NodeSpec, Schema, SchemaBuilder, Selection};
use taino_edit_dom::EditorView;
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, smart_enter_in_list, Lists, Paragraph,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, Text};

wasm_bindgen_test_configure!(run_in_browser);

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

/// `bullet_list > list_item > paragraph(text)` for each `items` string.
fn list_doc(s: &Schema, items: &[&str]) -> Node {
    let lis: Vec<Node> = items
        .iter()
        .map(|t| {
            let kids = if t.is_empty() {
                vec![]
            } else {
                vec![s.text(t, vec![]).unwrap()]
            };
            let p = s
                .node("paragraph", Default::default(), kids, vec![])
                .unwrap();
            s.create_node("list_item", Default::default(), vec![p], vec![])
                .unwrap()
        })
        .collect();
    let list = s
        .create_node("bullet_list", Default::default(), lis, vec![])
        .unwrap();
    s.node("doc", Default::default(), vec![list], vec![])
        .unwrap()
}

fn attach(doc: Node, s: Schema) -> (EditorView, Element) {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&root).unwrap();
    let view = EditorView::mount(doc, s, root.clone());
    (view, root)
}

fn cleanup(root: &Element) {
    let _ = root.parent_element().map(|b| b.remove_child(root));
}

/// Run `cmd` against `st`, returning the new state (or `st` unchanged).
fn run(st: EditorState, cmd: &Command) -> EditorState {
    let mut next = None;
    {
        let mut d = |tx| next = Some(st.apply(tx));
        cmd(&st, Some(&mut d));
    }
    next.unwrap_or(st)
}

/// Drive the view from a state change the way an adapter does.
fn sync(view: &mut EditorView, st: &EditorState) {
    view.update(st.doc().clone());
    let _ = view.set_selection(st.selection());
}

/// Run a key through the full (chained) keymap, like the demo's on_keydown.
fn press(st: EditorState, s: &Schema, key: &KeyPress) -> EditorState {
    let keymap = build_keymap_with(&[&Paragraph, &Lists], s, false);
    let mut next = None;
    {
        let mut d = |tx| next = Some(st.apply(tx));
        keymap.handle(&st, key, Some(&mut d));
    }
    next.unwrap_or(st)
}

fn caret(st: EditorState, pos: usize) -> EditorState {
    let mut t = st.tr();
    t.set_selection(Selection::caret(pos));
    st.apply(t)
}

#[wasm_bindgen_test]
fn enter_on_empty_item_exits_the_list() {
    let s = schema();
    // List with one item "ab"; caret at end (pos 5).
    let st = caret(EditorState::new(list_doc(&s, &["ab"]), s.clone()), 5);
    let (mut view, root) = attach(st.doc().clone(), s.clone());
    view.set_selection(st.selection()).ok();

    let enter = KeyPress::key("Enter");
    // Enter 1: split → empty trailing item.
    let st = press(st, &s, &enter);
    sync(&mut view, &st);
    assert_eq!(
        st.doc().child(0).child_count(),
        2,
        "two items after first Enter"
    );

    // Enter 2 on the empty item: exit the list.
    let st = press(st, &s, &enter);
    sync(&mut view, &st);

    // The empty item left the list; "ab" stays listed, a paragraph follows.
    let doc = st.doc();
    assert_eq!(doc.child(0).node_type().name(), "bullet_list");
    assert_eq!(
        doc.child(0).child_count(),
        1,
        "only 'ab' remains in the list"
    );
    assert!(
        doc.child_count() >= 2 && doc.child(1).node_type().name() == "paragraph",
        "an empty paragraph should follow the list: {:?}",
        doc.to_json()
    );
    // No corruption: text is intact and the DOM matches.
    assert_eq!(doc.text_content(), "ab");
    assert!(root.query_selector("p").unwrap().is_some());
    cleanup(&root);
}

#[wasm_bindgen_test]
fn backspace_at_start_of_list_item_lifts_it_out() {
    let s = schema();
    // Two items; caret at the START of the second item's text (pos 8).
    // Positions: ul@0, li1[1..7] (p"ab" [2..6]), li2@7, p2 content start 9...
    // wait: li1 size = p("ab")=4 +2 = 6 -> [1,7); li2@7, p2@8 content@9.
    let st = caret(EditorState::new(list_doc(&s, &["ab", "cd"]), s.clone()), 9);
    let (mut view, root) = attach(st.doc().clone(), s.clone());
    view.set_selection(st.selection()).ok();

    let st = press(st, &s, &KeyPress::key("Backspace"));
    sync(&mut view, &st);

    // Backspace at the start of a list item should lift it out of the list
    // (not delete a character, not corrupt). "cd" becomes a paragraph after
    // a one-item list of "ab".
    let doc = st.doc();
    assert_eq!(
        doc.text_content(),
        "abcd",
        "no characters lost: {:?}",
        doc.to_json()
    );
    assert_eq!(
        doc.child(0).child_count(),
        1,
        "first item stays in the list: {:?}",
        doc.to_json()
    );
    cleanup(&root);
}

#[wasm_bindgen_test]
fn empty_item_at_mount_has_trailing_break() {
    // Isolates render-at-mount: a list whose second item is empty from the
    // start. If THIS lacks the <br>, the bug is in render/is_textblock, not
    // in the patch path.
    let s = schema();
    let (_view, root) = attach(list_doc(&s, &["a", ""]), s);
    let html = root.inner_html();
    assert!(
        html.contains("<br"),
        "empty list item must render a trailing <br> at mount: {html}"
    );
    cleanup(&root);
}

#[wasm_bindgen_test]
fn enter_at_end_of_item_makes_a_focusable_empty_item() {
    let s = schema();
    // One item "Rust"; caret at end of its text (doc pos 7).
    let mut st = EditorState::new(list_doc(&s, &["Rust"]), s.clone());
    let mut t = st.tr();
    t.set_selection(Selection::caret(7));
    st = st.apply(t);

    let (mut view, root) = attach(st.doc().clone(), s.clone());
    view.set_selection(st.selection()).ok();

    // Enter → smart_enter_in_list splits into two items.
    let after = run(st, &smart_enter_in_list());
    sync(&mut view, &after);

    // Model: two list_items, the second empty.
    let list = after.doc().child(0);
    assert_eq!(list.child_count(), 2, "two list items in the model");
    assert_eq!(list.child(1).text_content(), "", "second item is empty");

    // DOM: two <li>, and the empty second one carries a trailing <br>.
    let html = root.inner_html();
    assert_eq!(root.query_selector_all("li").unwrap().length(), 2, "{html}");
    assert!(
        html.contains("<br"),
        "the empty list item needs a trailing <br> to be focusable: {html}"
    );

    // The caret was placed inside the empty item (set_selection succeeded).
    assert_eq!(
        view.read_selection(),
        Some(after.selection()),
        "DOM caret must round-trip to the command's caret inside the new item"
    );
    cleanup(&root);
}

#[wasm_bindgen_test]
fn typing_into_the_new_empty_item_is_captured_without_duplication() {
    let s = schema();
    let mut st = EditorState::new(list_doc(&s, &["Rust"]), s.clone());
    let mut t = st.tr();
    t.set_selection(Selection::caret(7));
    st = st.apply(t);

    let (mut view, root) = attach(st.doc().clone(), s.clone());
    view.set_selection(st.selection()).ok();
    let after = run(st, &smart_enter_in_list());
    sync(&mut view, &after);

    // Simulate typing "x" into the empty second item (before its <br>).
    let li2 = root.query_selector_all("li").unwrap().item(1).unwrap();
    let p2: Element = li2.first_child().unwrap().dyn_into().unwrap();
    let document = web_sys::window().unwrap().document().unwrap();
    let typed: Text = document.create_text_node("x");
    match p2.first_child() {
        Some(br) => {
            p2.insert_before(&typed, Some(&br)).unwrap();
        }
        None => {
            p2.append_child(&typed).unwrap();
        }
    }

    let transform = view
        .read_dom_changes()
        .expect("typing into the empty list item is detected");
    let mut tx = after.tr();
    for step in transform.steps() {
        let _ = tx.transform().step(step.clone(), &s);
    }
    let typed_state = after.apply(tx);
    sync(&mut view, &typed_state);

    // Model: second item now reads "x"; DOM has no duplicate.
    assert_eq!(typed_state.doc().child(0).child(1).text_content(), "x");
    let li2 = root.query_selector_all("li").unwrap().item(1).unwrap();
    let p2: Element = li2.first_child().unwrap().dyn_into().unwrap();
    assert_eq!(
        p2.text_content().as_deref(),
        Some("x"),
        "no duplicated text in the list item: {}",
        root.inner_html()
    );
    cleanup(&root);
}
