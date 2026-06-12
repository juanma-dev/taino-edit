//! Phase 5 Unit B: event wiring inside `<TainoEditor>` — typing, IME, paste.
//! The component must turn DOM-side input events into committed transforms
//! on the state signal.

#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use taino_edit_core::{DomSpec, EditorState, Node, NodeSpec, Schema, SchemaBuilder, Selection};
use taino_edit_leptos::TainoEditor;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, HtmlElement, Text};

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

/// After mount + settle, return (editor_div, first_text_node).
fn dom_handles(host: &HtmlElement) -> (Element, Text) {
    let editor: Element = host.first_child().unwrap().dyn_into().unwrap();
    let p = editor.first_child().unwrap();
    let text: Text = p.first_child().unwrap().dyn_into().unwrap();
    (editor, text)
}

fn fire(target: &Element, name: &str) {
    let ev = web_sys::Event::new(name).unwrap();
    let _ = target.dispatch_event(&ev);
}

#[wasm_bindgen_test]
async fn typing_into_a_text_node_updates_the_state() {
    let host = host();
    let s = schema();
    let state = RwSignal::new(EditorState::new(
        doc(&s, vec![para(&s, "Hello")]),
        s.clone(),
    ));

    leptos::mount::mount_to(host.clone(), move || view! { <TainoEditor state=state /> }).forget();
    settle().await;

    let (editor, text) = dom_handles(&host);
    // Simulate the browser writing the typed character into the DOM text.
    text.set_data("Hello!");
    fire(&editor, "input");
    settle().await;

    assert_eq!(
        state.get_untracked().doc().text_content(),
        "Hello!",
        "state must follow DOM-side typing"
    );
}

#[wasm_bindgen_test]
async fn input_during_composition_does_not_commit() {
    let host = host();
    let s = schema();
    let state = RwSignal::new(EditorState::new(doc(&s, vec![para(&s, "Hello")]), s));

    leptos::mount::mount_to(host.clone(), move || view! { <TainoEditor state=state /> }).forget();
    settle().await;

    let (editor, text) = dom_handles(&host);
    fire(&editor, "compositionstart");

    // While composing, several transient glyphs land in the DOM and fire
    // `input` events. None of them must reach the state signal.
    for transient in ["Hellou", "Helloux", "こんにちは"] {
        text.set_data(transient);
        fire(&editor, "input");
        settle().await;
        assert_eq!(
            state.get_untracked().doc().text_content(),
            "Hello",
            "state must stay frozen at `Hello` while composing (saw `{transient}`)"
        );
    }

    // compositionend commits the final glyphs in one transform.
    fire(&editor, "compositionend");
    settle().await;
    assert_eq!(state.get_untracked().doc().text_content(), "こんにちは");
}

#[wasm_bindgen_test]
async fn drag_extension_after_selection_mirror_is_not_clipped() {
    // Regression: mid drag-select, a `selectionchange` mirrors the partial
    // range into state; the reactive effect then ran *after* the user had
    // already extended the selection, and wrote the stale partial range back
    // into the browser — clipping the selection's tail, so a subsequent
    // toggle_mark/set_block_type missed the trailing word(s).
    let host = host();
    let s = schema();
    let state = RwSignal::new(EditorState::new(
        doc(&s, vec![para(&s, "hello world")]),
        s.clone(),
    ));

    leptos::mount::mount_to(host.clone(), move || view! { <TainoEditor state=state /> }).forget();
    settle().await;

    let (editor, text) = dom_handles(&host);
    let _ = editor.unchecked_ref::<HtmlElement>().focus();

    let dom_sel = web_sys::window().unwrap().get_selection().unwrap().unwrap();
    // Mid-drag: "hello" is selected and the browser delivers selectionchange.
    dom_sel.set_base_and_extent(&text, 0, &text, 5).unwrap();
    let document = web_sys::window().unwrap().document().unwrap();
    let ev = web_sys::Event::new("selectionchange").unwrap();
    // The mirror listener runs synchronously on dispatch.
    let _ = document.dispatch_event(&ev);
    // Before the effect runs, the drag extends over the second word.
    dom_sel.set_base_and_extent(&text, 0, &text, 11).unwrap();

    settle().await; // effect + the browser's own queued selectionchange
    settle().await;

    let dom_sel = web_sys::window().unwrap().get_selection().unwrap().unwrap();
    assert_eq!(
        dom_sel.focus_offset(),
        11,
        "the effect must not clip a selection the user extended after the mirror"
    );
    assert_eq!(
        state.get_untracked().selection(),
        Selection::Text {
            anchor: 1,
            head: 12
        },
        "state must converge on the full selection"
    );
}

#[wasm_bindgen_test]
async fn cleanup_detaches_listeners() {
    // Mount, then immediately unmount via a wrapping `Show`. After unmount
    // the input listener must not feed stale state.
    let host = host();
    let s = schema();
    let state = RwSignal::new(EditorState::new(
        doc(&s, vec![para(&s, "Hello")]),
        s.clone(),
    ));
    let visible = RwSignal::new(true);

    leptos::mount::mount_to(host.clone(), move || {
        view! {
            <Show when=move || visible.get() fallback=|| view! { <p>gone</p> }>
                <TainoEditor state=state />
            </Show>
        }
    })
    .forget();
    settle().await;

    let snapshot_before_unmount = state.get_untracked().doc().text_content();
    visible.set(false);
    settle().await;

    // Dispatch an input on the now-orphaned host: nothing must touch state.
    // (Editor is gone, so the host's children are the fallback.)
    fire(&host, "input");
    settle().await;
    assert_eq!(
        state.get_untracked().doc().text_content(),
        snapshot_before_unmount
    );
}
