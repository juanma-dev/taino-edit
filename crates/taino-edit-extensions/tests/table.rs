//! v0.3 — the `Table` structural extension.

use taino_edit_core::{
    AttrValue, Command, EditorState, NodeSpec, Schema, SchemaBuilder, Selection,
};
use taino_edit_extensions::{
    add_column_after, add_column_before, add_row_after, build_schema_with, delete_column,
    delete_row, delete_table, insert_table, toggle_header_cell, toggle_header_column,
    toggle_header_row, Extension, Paragraph, Table,
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
    build_schema_with(base, &[&Paragraph, &Table], "doc").unwrap()
}

/// A doc with a single paragraph; caret at position 1 (inside it).
fn doc_with_paragraph() -> EditorState {
    let s = schema();
    let t = s.text("hi", vec![]).unwrap();
    let p = s
        .node("paragraph", Default::default(), vec![t], vec![])
        .unwrap();
    let doc = s.node("doc", Default::default(), vec![p], vec![]).unwrap();
    let st = EditorState::new(doc, s);
    let mut tr = st.tr();
    tr.set_selection(Selection::caret(1));
    st.apply(tr)
}

/// Count `<tr>` and per-row `<td>` from the serialized HTML.
fn dims(html: &str) -> (usize, usize) {
    let rows = html.matches("<tr>").count();
    let first_row = html.split("<tr>").nth(1).unwrap_or("");
    let cols = first_row
        .split("</tr>")
        .next()
        .unwrap_or("")
        .matches("<td>")
        .count();
    (rows, cols)
}

#[test]
fn table_registers_three_nodes() {
    let adds = Table.schema_additions();
    let names: Vec<&str> = adds.nodes.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["table_cell", "table_row", "table"]);
}

#[test]
fn insert_table_creates_grid_after_block() {
    let s = run(doc_with_paragraph(), &insert_table(2, 3));
    let html = s.doc().to_html();
    assert!(html.contains("<table>"), "expected a table: {html}");
    assert_eq!(dims(&html), (2, 3), "expected 2x3 grid: {html}");
    // The original paragraph survives before the table.
    assert!(html.starts_with("<p>hi</p><table>"), "got: {html}");
}

#[test]
fn insert_table_places_caret_in_first_cell() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    // Caret should sit inside the first cell's paragraph; adding a column
    // "after" should then operate on column 0 → table becomes 3 wide.
    let s = run(s, &add_column_after());
    assert_eq!(dims(&s.doc().to_html()), (2, 3));
}

#[test]
fn add_row_after_grows_row_count() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    let s = run(s, &add_row_after());
    assert_eq!(dims(&s.doc().to_html()), (3, 2));
}

#[test]
fn add_column_before_and_after() {
    let s = run(doc_with_paragraph(), &insert_table(1, 1));
    let s = run(s, &add_column_after());
    assert_eq!(dims(&s.doc().to_html()), (1, 2));
    let s = run(s, &add_column_before());
    assert_eq!(dims(&s.doc().to_html()), (1, 3));
}

#[test]
fn delete_row_shrinks_then_deletes_table() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    let s = run(s, &delete_row());
    assert_eq!(dims(&s.doc().to_html()), (1, 2));
    // Deleting the last row removes the whole table.
    let s = run(s, &delete_row());
    assert!(!s.doc().to_html().contains("<table>"));
    assert!(s.doc().to_html().contains("<p>hi</p>"));
}

#[test]
fn delete_column_shrinks_then_deletes_table() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    let s = run(s, &delete_column());
    assert_eq!(dims(&s.doc().to_html()), (2, 1));
    let s = run(s, &delete_column());
    assert!(!s.doc().to_html().contains("<table>"));
}

#[test]
fn delete_table_removes_it() {
    let s = run(doc_with_paragraph(), &insert_table(3, 3));
    assert!(s.doc().to_html().contains("<table>"));
    let s = run(s, &delete_table());
    assert!(!s.doc().to_html().contains("<table>"));
    assert!(s.doc().to_html().contains("<p>hi</p>"));
}

#[test]
fn table_commands_are_noop_outside_a_table() {
    let s = doc_with_paragraph();
    // Caret is in a plain paragraph — column/row/delete ops shouldn't apply.
    assert!(!add_column_after()(&s, None));
    assert!(!add_row_after()(&s, None));
    assert!(!delete_row()(&s, None));
    assert!(!delete_table()(&s, None));
}

#[test]
fn cell_declares_colspan_rowspan_header_attrs() {
    let adds = Table.schema_additions();
    let (_, cell_spec) = &adds.nodes[0];
    assert!(cell_spec.attrs.contains_key("colspan"));
    assert!(cell_spec.attrs.contains_key("rowspan"));
    assert!(cell_spec.attrs.contains_key("header"));
}

#[test]
fn toggle_header_row_emits_th() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    // Caret is in cell (0,0); toggle the header on its row.
    let s = run(s, &toggle_header_row());
    let html = s.doc().to_html();
    // First row's cells become <th>; second row stays <td>.
    let first_row = html.split("<tr>").nth(1).unwrap();
    assert_eq!(
        first_row.matches("<th>").count(),
        2,
        "row 0 should be all th: {html}"
    );
    let second_row = html.split("<tr>").nth(2).unwrap();
    assert!(second_row.contains("<td>"), "row 1 should stay td: {html}");

    // Toggling again flips back to <td>.
    let s = run(s, &toggle_header_row());
    assert!(!s.doc().to_html().contains("<th>"));
}

#[test]
fn toggle_header_column_emits_th_in_each_row() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    let s = run(s, &toggle_header_column());
    let html = s.doc().to_html();
    // Column 0 of both rows becomes <th>.
    assert_eq!(
        html.matches("<th>").count(),
        2,
        "expected 2 th (one per row): {html}"
    );
}

#[test]
fn toggle_header_cell_emits_single_th() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    let s = run(s, &toggle_header_cell());
    let html = s.doc().to_html();
    assert_eq!(
        html.matches("<th>").count(),
        1,
        "expected exactly 1 th: {html}"
    );
}

#[test]
fn header_round_trips_through_html() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    let s = run(s, &toggle_header_row());
    let html = s.doc().to_html();
    let parsed = s.schema().parse_html(&html).expect("parse");
    // The reparsed first row's first cell must still be a header.
    let table = parsed
        .content()
        .iter()
        .find(|n| n.node_type().name() == "table")
        .expect("table present");
    let first_cell = table.child(0).child(0);
    assert_eq!(
        first_cell.attrs().get("header"),
        Some(&AttrValue::from(true)),
        "header attr lost on round-trip"
    );
}

#[test]
fn table_round_trips_through_html() {
    let s = run(doc_with_paragraph(), &insert_table(2, 2));
    let html = s.doc().to_html();
    let parsed = s.schema().parse_html(&html).expect("parse");
    // The reparsed doc must still contain a table with the same dims.
    let reparsed_html = parsed.to_html();
    assert_eq!(
        dims(&reparsed_html),
        (2, 2),
        "round-trip changed dims: {reparsed_html}"
    );
}
