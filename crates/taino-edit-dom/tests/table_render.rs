//! v0.3 hardening (H6): table rendering + incremental patch in a real
//! headless-Chromium contenteditable. Uses the actual `Table` extension
//! schema so the `<table>/<tr>/<td>/<th>` + colspan/rowspan DOM output is
//! exercised end-to-end, not just the host-side model.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{AttrValue, Attrs, Node, NodeSpec, Schema, SchemaBuilder, Selection};
use taino_edit_dom::EditorView;
use taino_edit_extensions::{build_schema_with, Paragraph, Table};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::HtmlElement;

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

fn cell(s: &Schema, text: &str, attrs: Attrs) -> Node {
    let txt = s.text(text, vec![]).unwrap();
    let p = s
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    s.node("table_cell", attrs, vec![p], vec![]).unwrap()
}

fn row(s: &Schema, cells: Vec<Node>) -> Node {
    s.node("table_row", Default::default(), cells, vec![]).unwrap()
}

fn table(s: &Schema, rows: Vec<Node>) -> Node {
    s.node("table", Default::default(), rows, vec![]).unwrap()
}

fn doc(s: &Schema, table: Node) -> Node {
    s.node("doc", Default::default(), vec![table], vec![]).unwrap()
}

fn mount(doc: Node, s: Schema) -> EditorView {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    EditorView::mount(doc, s, root)
}

fn inner(view: &EditorView) -> String {
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    html.inner_html()
}

#[wasm_bindgen_test]
fn renders_table_tr_td_into_dom() {
    let s = schema();
    let t = table(
        &s,
        vec![
            row(&s, vec![cell(&s, "a", Attrs::new()), cell(&s, "b", Attrs::new())]),
            row(&s, vec![cell(&s, "c", Attrs::new()), cell(&s, "d", Attrs::new())]),
        ],
    );
    let view = mount(doc(&s, t), s);
    let html = inner(&view);
    assert!(html.contains("<table>"), "table element present: {html}");
    assert_eq!(html.matches("<tr>").count(), 2, "two rows: {html}");
    assert_eq!(html.matches("<td>").count(), 4, "four cells: {html}");
    assert!(html.contains("<p>a</p>"), "cell content rendered: {html}");
}

#[wasm_bindgen_test]
fn header_cell_renders_as_th() {
    let s = schema();
    let mut hattrs = Attrs::new();
    hattrs.insert("header".into(), AttrValue::from(true));
    let t = table(
        &s,
        vec![row(
            &s,
            vec![cell(&s, "H", hattrs), cell(&s, "x", Attrs::new())],
        )],
    );
    let view = mount(doc(&s, t), s);
    let html = inner(&view);
    // Cell content is wrapped in a paragraph: <th><p>H</p></th>.
    assert!(html.contains("<th><p>H</p></th>"), "header cell is <th>: {html}");
    assert!(html.contains("<td><p>x</p></td>"), "normal cell is <td>: {html}");
}

#[wasm_bindgen_test]
fn colspan_and_rowspan_render_in_dom() {
    let s = schema();
    let mut span = Attrs::new();
    span.insert("colspan".into(), AttrValue::from(2u64));
    span.insert("rowspan".into(), AttrValue::from(2u64));
    let t = table(&s, vec![row(&s, vec![cell(&s, "big", span)])]);
    let view = mount(doc(&s, t), s);
    let html = inner(&view);
    assert!(html.contains("colspan=\"2\""), "colspan rendered: {html}");
    assert!(html.contains("rowspan=\"2\""), "rowspan rendered: {html}");
}

#[wasm_bindgen_test]
fn colwidth_renders_as_style_width() {
    let s = schema();
    let mut w = Attrs::new();
    w.insert("colwidth".into(), AttrValue::from(150u64));
    let t = table(&s, vec![row(&s, vec![cell(&s, "x", w)])]);
    let view = mount(doc(&s, t), s);
    let html = inner(&view);
    assert!(
        html.contains("width: 150px"),
        "colwidth → style width: {html}"
    );
}

#[wasm_bindgen_test]
fn update_patches_table_dom_when_a_row_is_added() {
    let s = schema();
    let t1 = table(
        &s,
        vec![row(&s, vec![cell(&s, "a", Attrs::new())])],
    );
    let mut view = mount(doc(&s, t1), s.clone());
    assert_eq!(inner(&view).matches("<tr>").count(), 1);

    // Reconcile to a two-row table — the DOM should patch in place.
    let t2 = table(
        &s,
        vec![
            row(&s, vec![cell(&s, "a", Attrs::new())]),
            row(&s, vec![cell(&s, "b", Attrs::new())]),
        ],
    );
    view.update(doc(&s, t2));
    let html = inner(&view);
    assert_eq!(html.matches("<tr>").count(), 2, "row added in DOM: {html}");
    assert!(html.contains("<p>b</p>"), "new row content: {html}");
}

#[wasm_bindgen_test]
fn selection_round_trips_into_a_deep_table_cell() {
    // A caret deep inside the last cell must survive the
    // position → DOM → position round-trip through the nested
    // table>tr>td>p>text structure (the real browser risk for cell
    // navigation, which moves the caret by absolute position). The
    // browser Selection API only works on attached DOM, so mount into
    // <body>.
    let s = schema();
    let t = table(
        &s,
        vec![
            row(&s, vec![cell(&s, "a", Attrs::new()), cell(&s, "b", Attrs::new())]),
            row(&s, vec![cell(&s, "c", Attrs::new()), cell(&s, "d", Attrs::new())]),
        ],
    );
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&root).unwrap();
    let view = EditorView::mount(doc(&s, t), s, root.clone());

    // Position 21 is the caret at the start of "d" in cell (1,1).
    view.set_selection(Selection::caret(21)).unwrap();
    let read = view.read_selection().expect("a selection is present");
    assert_eq!(read.from(), 21, "caret round-trips into the last cell: {read:?}");

    let _ = document.body().unwrap().remove_child(&root);
}

#[wasm_bindgen_test]
fn update_patches_th_toggle() {
    let s = schema();
    let t1 = table(&s, vec![row(&s, vec![cell(&s, "x", Attrs::new())])]);
    let mut view = mount(doc(&s, t1), s.clone());
    assert!(inner(&view).contains("<td><p>x</p></td>"));

    let mut hattrs = Attrs::new();
    hattrs.insert("header".into(), AttrValue::from(true));
    let t2 = table(&s, vec![row(&s, vec![cell(&s, "x", hattrs)])]);
    view.update(doc(&s, t2));
    let html = inner(&view);
    assert!(html.contains("<th><p>x</p></th>"), "td→th patched: {html}");
    assert!(!html.contains("<td"), "no stale td remains: {html}");
}
