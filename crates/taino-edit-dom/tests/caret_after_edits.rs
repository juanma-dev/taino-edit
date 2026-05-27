//! v0.5 hardening: the caret must land where a structural command says it
//! should after `view.update` + `set_selection` — reproducing the "type after
//! join and the caret jumps to the end" report.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{
    join_backward, split_block, Command, DomSpec, EditorState, Node, NodeSpec, Schema,
    SchemaBuilder, Selection,
};
use taino_edit_dom::EditorView;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

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
                to_dom: Some(|_| DomSpec::element("p")),
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

fn para(s: &Schema, t: &str) -> Node {
    s.node(
        "paragraph",
        Default::default(),
        vec![s.text(t, vec![]).unwrap()],
        vec![],
    )
    .unwrap()
}

fn doc(s: &Schema, ps: Vec<Node>) -> Node {
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

fn attach(d: Node, s: Schema) -> (EditorView, web_sys::Element) {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&root).unwrap();
    let view = EditorView::mount(d, s, root.clone());
    (view, root)
}

fn run(st: EditorState, cmd: &Command) -> EditorState {
    let mut next = None;
    {
        let mut d = |tx| next = Some(st.apply(tx));
        cmd(&st, Some(&mut d));
    }
    next.unwrap_or(st)
}

fn sync(view: &mut EditorView, st: &EditorState) {
    view.update(st.doc().clone());
    let _ = view.set_selection(st.selection());
}

#[wasm_bindgen_test]
fn caret_round_trips_through_split_then_join() {
    let s = schema();
    // "abc"; caret after "ab" (pos 3).
    let st = {
        let st = EditorState::new(doc(&s, vec![para(&s, "abc")]), s.clone());
        let mut t = st.tr();
        t.set_selection(Selection::caret(3));
        st.apply(t)
    };
    let (mut view, root) = attach(st.doc().clone(), s.clone());
    view.set_selection(st.selection()).ok();

    // Enter → split into "ab" | "c", caret at start of "c".
    let split: Command = Box::new(split_block);
    let st = run(st, &split);
    sync(&mut view, &st);
    assert_eq!(st.doc().child_count(), 2, "split produced two paragraphs");

    // Backspace → join back to "abc", caret at the join point (pos 3).
    let join: Command = Box::new(join_backward);
    let st = run(st, &join);
    sync(&mut view, &st);
    assert_eq!(st.doc(), &doc(&s, vec![para(&s, "abc")]), "joined back");
    assert_eq!(
        st.selection(),
        Selection::caret(3),
        "model caret sits at the join point"
    );

    // The DOM caret must match the model — not drift to the end.
    assert_eq!(
        view.read_selection(),
        Some(Selection::Text { anchor: 3, head: 3 }),
        "DOM caret must be at the join point, not the end of the paragraph"
    );
    let _ = root.parent_element().map(|b| b.remove_child(&root));
}
