//! v0.5: `<TainoEditor>` owns keyboard editing. A real `keydown` runs the
//! keymap command and applies it synchronously, so the model updates within
//! the handler (no async lag).

#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use taino_edit_core::{
    base_keymap, DomSpec, EditorState, Node, NodeSpec, Schema, SchemaBuilder, Selection,
};
use taino_edit_leptos::TainoEditor;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, HtmlElement, KeyboardEvent, KeyboardEventInit};

wasm_bindgen_test_configure!(run_in_browser);

async fn settle() {
    TimeoutFuture::new(0).await;
}

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

fn host() -> HtmlElement {
    let document = web_sys::window().unwrap().document().unwrap();
    let host = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&host).unwrap();
    host.unchecked_into()
}

fn press_key(el: &Element, key: &str) {
    let init = KeyboardEventInit::new();
    init.set_key(key);
    init.set_bubbles(true);
    let ev = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap();
    let _ = el.dispatch_event(&ev);
}

#[wasm_bindgen_test]
async fn keydown_enter_splits_the_block_synchronously() {
    let host = host();
    let s = schema();
    // "abc" with the caret after "ab" (pos 3).
    let st = {
        let st = EditorState::new(doc(&s, vec![para(&s, "abc")]), s.clone());
        let mut tx = st.tr();
        tx.set_selection(Selection::caret(3));
        st.apply(tx)
    };
    let state = RwSignal::new(st);
    leptos::mount::mount_to(host.clone(), move || {
        view! { <TainoEditor state=state keymap=base_keymap(false) /> }
    })
    .forget();
    settle().await;

    // The editor div is the host's first child.
    let editor: Element = host.first_child().unwrap().dyn_into().unwrap();
    press_key(&editor, "Enter");

    // The split has already been applied to the model (synchronous handler).
    let d = state.get_untracked();
    assert_eq!(
        d.doc().child_count(),
        2,
        "Enter must split into two paragraphs"
    );
    assert_eq!(d.doc().child(0).text_content(), "ab");
    assert_eq!(d.doc().child(1).text_content(), "c");
}

#[wasm_bindgen_test]
async fn keydown_without_keymap_does_not_panic() {
    let host = host();
    let s = schema();
    let state = RwSignal::new(EditorState::new(doc(&s, vec![para(&s, "x")]), s));
    leptos::mount::mount_to(host.clone(), move || view! { <TainoEditor state=state /> }).forget();
    settle().await;
    let editor: Element = host.first_child().unwrap().dyn_into().unwrap();
    // No keymap installed: keydown is a no-op (the doc is unchanged).
    press_key(&editor, "Enter");
    assert_eq!(state.get_untracked().doc().child_count(), 1);
}
