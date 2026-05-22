//! v0.3 hardening (H7a): the DOM-aware `ViewPlugin` infrastructure —
//! nested-node decorations, plugin event dispatch + decoration refresh, and
//! the `pos_at_point` primitive — exercised in headless Chromium.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{Attrs, Node, NodeSpec, Schema, SchemaBuilder, Selection};
use taino_edit_dom::{Decoration, EditorView, ViewAction, ViewPlugin};
use taino_edit_extensions::{build_schema_with, Paragraph, Table};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, HtmlElement};

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
    build_schema_with(base, &[&Paragraph, &Table], "doc").unwrap()
}

fn cell(s: &Schema, t: &str) -> Node {
    let txt = s.text(t, vec![]).unwrap();
    let p = s
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    s.node("table_cell", Attrs::new(), vec![p], vec![]).unwrap()
}

fn table_doc(s: &Schema) -> Node {
    let r0 = s
        .node(
            "table_row",
            Default::default(),
            vec![cell(s, "a"), cell(s, "b")],
            vec![],
        )
        .unwrap();
    let r1 = s
        .node(
            "table_row",
            Default::default(),
            vec![cell(s, "c"), cell(s, "d")],
            vec![],
        )
        .unwrap();
    let t = s
        .node("table", Default::default(), vec![r0, r1], vec![])
        .unwrap();
    s.node("doc", Default::default(), vec![t], vec![]).unwrap()
}

fn mount_attached(doc: Node, s: Schema) -> (EditorView, Element) {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&root).unwrap();
    let view = EditorView::mount(doc, s, root.clone());
    (view, root)
}

#[wasm_bindgen_test]
fn node_decoration_targets_a_nested_cell() {
    let s = schema();
    let mut view = EditorView::mount(table_doc(&s), s, {
        web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("div")
            .unwrap()
    });
    // Position 19 is directly before cell (1,1) ("d"); decorate it.
    view.set_decorations(vec![Decoration::Node {
        pos: 19,
        class: "sel".into(),
    }]);
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    let inner = html.inner_html();
    assert!(
        inner.contains("<td class=\"sel\"><p>d</p></td>"),
        "decoration must land on the nested cell: {inner}"
    );
}

#[derive(Default)]
struct DummyPlugin;

impl ViewPlugin for DummyPlugin {
    fn handle_event(&self, _view: &EditorView, _ev: &web_sys::Event) -> Option<ViewAction> {
        Some(ViewAction::Select(Selection::caret(7)))
    }
    fn decorations(&self, _view: &EditorView, _sel: Option<Selection>) -> Vec<Decoration> {
        vec![Decoration::Node {
            pos: 2,
            class: "plugin-deco".into(),
        }]
    }
}

#[wasm_bindgen_test]
fn view_plugin_event_dispatch_returns_action() {
    let s = schema();
    let mut view = EditorView::mount(table_doc(&s), s, {
        web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("div")
            .unwrap()
    });
    view.set_view_plugins(vec![Box::new(DummyPlugin)]);
    let ev = web_sys::Event::new("mousedown").unwrap();
    match view.handle_view_event(&ev) {
        Some(ViewAction::Select(sel)) => assert_eq!(sel, Selection::caret(7)),
        Some(ViewAction::Command(_)) => panic!("expected a Select action, got Command"),
        None => panic!("expected the plugin to produce an action"),
    }
}

#[wasm_bindgen_test]
fn refresh_view_decorations_applies_plugin_output() {
    let s = schema();
    let mut view = EditorView::mount(table_doc(&s), s, {
        web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("div")
            .unwrap()
    });
    view.set_view_plugins(vec![Box::new(DummyPlugin)]);
    view.refresh_view_decorations(None);
    // pos 2 is the first cell (0,0); the plugin decorates it.
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    assert!(
        html.inner_html().contains("class=\"plugin-deco\""),
        "plugin decorations should be applied: {}",
        html.inner_html()
    );
}

#[wasm_bindgen_test]
fn pos_at_point_maps_into_a_cell() {
    let s = schema();
    let (view, root) = mount_attached(table_doc(&s), s);
    // Centre of the last cell's <td>.
    let td = root
        .query_selector_all("td")
        .unwrap()
        .item(3)
        .unwrap()
        .dyn_into::<Element>()
        .unwrap();
    let rect = td.get_bounding_client_rect();
    let x = (rect.left() + rect.width() / 2.0) as f32;
    let y = (rect.top() + rect.height() / 2.0) as f32;
    let pos = view.pos_at_point(x, y);
    // The whole table spans positions 1..25; a hit inside the last cell must
    // resolve to a position within that range (proving the DOM walk + node
    // identity match works), not None.
    assert!(pos.is_some(), "pos_at_point over a cell must resolve");
    let p = pos.unwrap();
    assert!(
        (1..26).contains(&p),
        "position {p} should be inside the table"
    );

    let _ = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .body()
        .unwrap()
        .remove_child(&root);
}
