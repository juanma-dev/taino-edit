//! v0.3 hardening (H7b): the `TableView` pointer plugin — cell drag-select,
//! selection highlight and column resize — driven by simulated mouse events
//! in headless Chromium.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{Attrs, Node, NodeSpec, Schema, SchemaBuilder, Selection};
use taino_edit_dom::{EditorView, ViewAction};
use taino_edit_extensions::{build_schema_with, Paragraph, Table};
use taino_edit_table_view::{TableView, SELECTED_CELL_CLASS};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, HtmlElement, MouseEvent, MouseEventInit};

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

/// Give cells a realistic size so a cell's centre isn't within the
/// resize-grip zone of its border (as it would be for tiny unstyled cells).
fn inject_table_css() {
    let document = web_sys::window().unwrap().document().unwrap();
    if document.get_element_by_id("taino-test-css").is_some() {
        return;
    }
    let style = document.create_element("style").unwrap();
    style.set_id("taino-test-css");
    style.set_text_content(Some(
        "td{min-width:60px;height:24px;padding:8px;box-sizing:content-box;border:1px solid #000}",
    ));
    document.head().unwrap().append_child(&style).unwrap();
}

fn mount() -> (EditorView, Element, Schema) {
    inject_table_css();
    let s = schema();
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&root).unwrap();
    let mut view = EditorView::mount(table_doc(&s), s.clone(), root.clone());
    view.set_view_plugins(vec![Box::new(TableView::new())]);
    (view, root, s)
}

fn cleanup(root: &Element) {
    let _ = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .body()
        .unwrap()
        .remove_child(root);
}

/// Center point of the nth `<td>` in the editor.
fn td_center(root: &Element, n: u32) -> (f64, f64) {
    let td = root
        .query_selector_all("td")
        .unwrap()
        .item(n)
        .unwrap()
        .dyn_into::<Element>()
        .unwrap();
    let r = td.get_bounding_client_rect();
    (r.left() + r.width() / 2.0, r.top() + r.height() / 2.0)
}

fn mouse_at(kind: &str, x: f64, y: f64) -> web_sys::Event {
    let init = MouseEventInit::new();
    init.set_client_x(x as i32);
    init.set_client_y(y as i32);
    init.set_bubbles(true);
    MouseEvent::new_with_mouse_event_init_dict(kind, &init)
        .unwrap()
        .dyn_into::<web_sys::Event>()
        .unwrap()
}

#[wasm_bindgen_test]
fn drag_across_cells_produces_a_cell_selection() {
    let (view, root, _s) = mount();
    let (x0, y0) = td_center(&root, 0); // cell (0,0)
    let (x1, y1) = td_center(&root, 3); // cell (1,1)

    // Press in cell 0 — no action yet (could be a caret).
    assert!(view
        .handle_view_event(&mouse_at("mousedown", x0, y0))
        .is_none());
    // Move to cell 3 — a rectangular cell selection.
    match view.handle_view_event(&mouse_at("mousemove", x1, y1)) {
        Some(ViewAction::Select(Selection::Cell { .. })) => {}
        _ => panic!("dragging across cells should yield a Cell selection"),
    }
    cleanup(&root);
}

#[wasm_bindgen_test]
fn no_drag_within_one_cell() {
    let (view, root, _s) = mount();
    let (x0, y0) = td_center(&root, 0);
    view.handle_view_event(&mouse_at("mousedown", x0, y0));
    // Move within the same cell → no cell selection.
    assert!(
        view.handle_view_event(&mouse_at("mousemove", x0 + 1.0, y0))
            .is_none(),
        "a move inside one cell must not start a cell selection"
    );
    cleanup(&root);
}

#[wasm_bindgen_test]
fn selection_highlight_decorations_mark_selected_cells() {
    let (mut view, root, _s) = mount();
    // Build a cell selection over the top row by driving the plugin.
    let (x0, y0) = td_center(&root, 0);
    let (x1, y1) = td_center(&root, 1);
    view.handle_view_event(&mouse_at("mousedown", x0, y0));
    let action = view.handle_view_event(&mouse_at("mousemove", x1, y1));
    let sel = match action {
        Some(ViewAction::Select(sel)) => sel,
        _ => panic!("expected a cell selection"),
    };
    // Refresh decorations for that selection — the covered cells get the class.
    view.refresh_view_decorations(Some(sel));
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    let inner = html.inner_html();
    assert_eq!(
        inner.matches(SELECTED_CELL_CLASS).count(),
        2,
        "both top-row cells should be highlighted: {inner}"
    );
    cleanup(&root);
}

#[wasm_bindgen_test]
fn resize_grip_emits_a_set_column_width_command() {
    let (view, root, _s) = mount();
    // Press near the right border of cell (0,0), then release further right.
    let td0 = root
        .query_selector_all("td")
        .unwrap()
        .item(0)
        .unwrap()
        .dyn_into::<Element>()
        .unwrap();
    let r = td0.get_bounding_client_rect();
    let border_x = r.right() - 1.0;
    let y = r.top() + r.height() / 2.0;
    assert!(view
        .handle_view_event(&mouse_at("mousedown", border_x, y))
        .is_none());
    // Release 40px to the right → a resize command.
    match view.handle_view_event(&mouse_at("mouseup", border_x + 40.0, y)) {
        Some(ViewAction::Command(_)) => {}
        _ => panic!("releasing a resize grip should emit a set_column_width command"),
    }
    cleanup(&root);
}
