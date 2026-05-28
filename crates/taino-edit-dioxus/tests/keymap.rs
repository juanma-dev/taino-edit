//! v0.5 (Dioxus parity): `<TainoEditor>` owns keyboard editing in Dioxus too.
//! A real `keydown` runs the keymap and applies it synchronously, so the DOM
//! reflects the split inside the handler.

#![cfg(target_arch = "wasm32")]

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use taino_edit_core::{
    base_keymap, DomSpec, EditorState, Node, NodeSpec, Schema, SchemaBuilder, Selection,
};
use taino_edit_dioxus::{KeymapProp, TainoEditor};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{KeyboardEvent, KeyboardEventInit};

wasm_bindgen_test_configure!(run_in_browser);

async fn settle() {
    for _ in 0..12 {
        TimeoutFuture::new(8).await;
    }
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

fn host() -> web_sys::Element {
    let document = web_sys::window().unwrap().document().unwrap();
    let h = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&h).unwrap();
    h
}

fn launch(app: fn() -> Element) -> web_sys::Element {
    let h = host();
    let vdom = VirtualDom::new(app);
    dioxus_web::launch::launch_virtual_dom(
        vdom,
        dioxus_web::Config::new().rootelement(h.clone()),
    );
    h
}

fn press_key(el: &web_sys::Element, key: &str) {
    let init = KeyboardEventInit::new();
    init.set_key(key);
    init.set_bubbles(true);
    let ev = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap();
    let _ = el.dispatch_event(&ev);
}

/// "abc" with the caret after "ab"; pressing Enter should split into "ab|c".
#[component]
fn SplitApp() -> Element {
    let state = use_signal(|| {
        let s = schema();
        let st = EditorState::new(doc(&s, vec![para(&s, "abc")]), s);
        let mut tx = st.tr();
        tx.set_selection(Selection::caret(3));
        st.apply(tx)
    });
    rsx! {
        TainoEditor { state, keymap: KeymapProp::new(base_keymap(false)) }
    }
}

#[wasm_bindgen_test]
async fn keydown_enter_splits_the_block_synchronously() {
    let host = launch(SplitApp);
    settle().await;

    let editor: web_sys::Element = host
        .query_selector(".taino-editor")
        .unwrap()
        .expect("editor mounted")
        .dyn_into()
        .unwrap();
    press_key(&editor, "Enter");

    // The split has been applied synchronously: two <p>, "ab" and "c".
    let paras = editor.query_selector_all("p").unwrap();
    assert_eq!(
        paras.length(),
        2,
        "Enter must split into two paragraphs: {}",
        editor.inner_html()
    );
    let first: web_sys::Element = paras.item(0).unwrap().dyn_into().unwrap();
    let second: web_sys::Element = paras.item(1).unwrap().dyn_into().unwrap();
    assert_eq!(first.text_content().as_deref(), Some("ab"));
    assert_eq!(second.text_content().as_deref(), Some("c"));
}
