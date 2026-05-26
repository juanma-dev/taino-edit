//! Browser tests for the Dioxus `<TainoEditor>`: it mounts a document into a
//! real `dioxus-web` render tree in headless Chromium, enables
//! `contenteditable`, and — when a `TableView` is supplied through the
//! `plugins` prop — installs it so the table renders under the editor.
//!
//! These close the previously-deferred "formal `wasm_bindgen_test` browser
//! tests for the Dioxus adapter" item: the Leptos adapter already has
//! `tests/component.rs`; this is its Dioxus counterpart.

#![cfg(target_arch = "wasm32")]

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use taino_edit_core::{Attrs, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dioxus::{EditorState, TainoEditor, ViewPlugins};
use taino_edit_extensions::{build_schema_with, Paragraph, Table};
use taino_edit_table_view::TableView;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// The Dioxus app is launched via `spawn_local`; yield across several
/// macro-tasks so the initial render lands before we read the DOM.
async fn settle() {
    for _ in 0..12 {
        TimeoutFuture::new(8).await;
    }
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
    build_schema_with(base, &[&Paragraph, &Table], "doc").unwrap()
}

fn paragraph_doc(s: &Schema) -> Node {
    let txt = s.text("Hello Dioxus", vec![]).unwrap();
    let p = s
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    s.node("doc", Default::default(), vec![p], vec![]).unwrap()
}

fn cell(s: &Schema, t: &str) -> Node {
    let txt = s.text(t, vec![]).unwrap();
    let p = s
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    s.node("table_cell", Attrs::new(), vec![p], vec![]).unwrap()
}

fn table_doc(s: &Schema) -> Node {
    let row = s
        .node(
            "table_row",
            Default::default(),
            vec![cell(s, "a"), cell(s, "b")],
            vec![],
        )
        .unwrap();
    let table = s
        .node("table", Default::default(), vec![row], vec![])
        .unwrap();
    s.node("doc", Default::default(), vec![table], vec![])
        .unwrap()
}

/// A fresh host `<div>` attached to the document body for one app to render
/// into.
fn host() -> web_sys::Element {
    let document = web_sys::window().unwrap().document().unwrap();
    let host = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&host).unwrap();
    host
}

/// Mount `app` into a fresh host and return it once the launch is scheduled.
fn launch(app: fn() -> Element) -> web_sys::Element {
    let host = host();
    let vdom = VirtualDom::new(app);
    dioxus_web::launch::launch_virtual_dom(
        vdom,
        dioxus_web::Config::new().rootelement(host.clone()),
    );
    host
}

#[component]
fn ParagraphApp() -> Element {
    let state = use_signal(|| {
        let s = schema();
        EditorState::new(paragraph_doc(&s), s)
    });
    rsx! { TainoEditor { state } }
}

#[component]
fn TableApp() -> Element {
    let state = use_signal(|| {
        let s = schema();
        EditorState::new(table_doc(&s), s)
    });
    rsx! {
        TainoEditor {
            state,
            plugins: ViewPlugins::new(vec![Box::new(TableView::new())]),
        }
    }
}

#[wasm_bindgen_test]
async fn component_mounts_initial_document() {
    let host = launch(ParagraphApp);
    settle().await;

    let inner = host.inner_html();
    assert!(
        inner.contains("<p>Hello Dioxus</p>"),
        "expected mounted paragraph, got: {inner}"
    );
    // The editor div is the one `TainoEditor` makes contenteditable.
    let editor = host
        .query_selector(".taino-editor")
        .unwrap()
        .expect("editor div mounted");
    assert_eq!(
        editor.get_attribute("contenteditable").as_deref(),
        Some("true"),
        "TainoEditor must enable contenteditable"
    );
}

#[wasm_bindgen_test]
async fn component_with_table_view_plugin_renders_the_table() {
    let host = launch(TableApp);
    settle().await;

    let inner = host.inner_html();
    // The component accepted the `plugins` prop, installed the TableView at
    // mount, and the table rendered under the editor.
    assert!(
        inner.contains("<table>"),
        "expected a rendered table, got: {inner}"
    );
    let cells = host.query_selector_all("td").unwrap().length();
    assert_eq!(cells, 2, "the 1×2 table should render two cells: {inner}");
    // Sanity: the editor host is a contenteditable taino editor.
    let editor: web_sys::Element = host
        .query_selector(".taino-editor")
        .unwrap()
        .expect("editor div mounted")
        .dyn_into()
        .unwrap();
    assert!(editor.has_attribute("contenteditable"));
}
