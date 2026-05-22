//! Tables — `table` / `table_row` / `table_cell` nodes plus the command
//! vocabulary a table-aware editor needs.
//!
//! Cells carry `colspan` / `rowspan` / `header` attrs and round-trip as
//! `<table><tr><td>` / `<th>` (with the span attributes). Structural
//! commands (`insert_table`, add/delete rows and columns, `delete_table`,
//! the `toggle_header_*` family) operate on the cell containing the caret
//! and rebuild the enclosing `table` node wholesale — tables are small, so
//! the O(table size) rebuild is cheap and keeps the position arithmetic
//! obviously correct.
//!
//! Cell-range selection (`CellSelection`), merge/split and column resizing
//! build on this foundation in later phases.

use std::collections::HashMap;

use taino_edit_core::{
    AttrSpec, AttrValue, Attrs, Command, DomSpec, Fragment, HtmlElement, Node, NodeSpec, ParseRule,
    ResolvedPos, Schema, Selection, Slice,
};

use crate::{Extension, SchemaAdditions};

/// The table extension. Adds `table`, `table_row` and `table_cell` nodes.
/// No default keymap — the structural commands are exported for a host
/// toolbar to wire (cell navigation would collide with the Lists `Tab`
/// binding, so it's left to the host).
pub struct Table;

fn cell_attr_specs() -> HashMap<String, AttrSpec> {
    let mut a = HashMap::new();
    a.insert(
        "colspan".to_string(),
        AttrSpec {
            default: Some(AttrValue::from(1u64)),
        },
    );
    a.insert(
        "rowspan".to_string(),
        AttrSpec {
            default: Some(AttrValue::from(1u64)),
        },
    );
    a.insert(
        "header".to_string(),
        AttrSpec {
            default: Some(AttrValue::from(false)),
        },
    );
    a
}

fn cell_to_dom(n: &Node) -> DomSpec {
    let header = n
        .attrs()
        .get("header")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut spec = DomSpec::element(if header { "th" } else { "td" });
    let colspan = n
        .attrs()
        .get("colspan")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let rowspan = n
        .attrs()
        .get("rowspan")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    if colspan != 1 {
        spec = spec.attr("colspan", colspan.to_string());
    }
    if rowspan != 1 {
        spec = spec.attr("rowspan", rowspan.to_string());
    }
    spec
}

fn cell_attrs_from(el: &HtmlElement, header: bool) -> Option<Attrs> {
    let mut a = Attrs::new();
    a.insert("header".into(), AttrValue::from(header));
    let colspan = el
        .attr("colspan")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    let rowspan = el
        .attr("rowspan")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    a.insert("colspan".into(), AttrValue::from(colspan));
    a.insert("rowspan".into(), AttrValue::from(rowspan));
    Some(a)
}

fn td_attrs(el: &HtmlElement) -> Option<Attrs> {
    cell_attrs_from(el, false)
}
fn th_attrs(el: &HtmlElement) -> Option<Attrs> {
    cell_attrs_from(el, true)
}

impl Extension for Table {
    fn name(&self) -> &str {
        "table"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions {
            nodes: vec![
                (
                    "table_cell".to_string(),
                    NodeSpec {
                        content: Some("block+".into()),
                        attrs: cell_attr_specs(),
                        to_dom: Some(cell_to_dom),
                        parse_dom: vec![
                            ParseRule::with_attrs("td", td_attrs),
                            ParseRule::with_attrs("th", th_attrs),
                        ],
                        ..Default::default()
                    },
                ),
                (
                    "table_row".to_string(),
                    NodeSpec {
                        content: Some("table_cell+".into()),
                        to_dom: Some(|_| DomSpec::element("tr")),
                        parse_dom: vec![ParseRule::tag("tr")],
                        ..Default::default()
                    },
                ),
                (
                    "table".to_string(),
                    NodeSpec {
                        content: Some("table_row+".into()),
                        group: Some("block".into()),
                        to_dom: Some(|_| DomSpec::element("table")),
                        parse_dom: vec![ParseRule::tag("table")],
                        ..Default::default()
                    },
                ),
            ],
            ..Default::default()
        }
    }
}

// ---- construction helpers ------------------------------------------------

fn empty_cell(schema: &Schema) -> Option<taino_edit_core::Node> {
    let p = schema
        .node("paragraph", Default::default(), vec![], vec![])
        .ok()?;
    schema
        .node("table_cell", Default::default(), vec![p], vec![])
        .ok()
}

fn build_table(schema: &Schema, rows: usize, cols: usize) -> Option<taino_edit_core::Node> {
    let mut row_nodes = Vec::with_capacity(rows);
    for _ in 0..rows {
        let mut cells = Vec::with_capacity(cols);
        for _ in 0..cols {
            cells.push(empty_cell(schema)?);
        }
        row_nodes.push(
            schema
                .node("table_row", Default::default(), cells, vec![])
                .ok()?,
        );
    }
    schema
        .node("table", Default::default(), row_nodes, vec![])
        .ok()
}

/// Locate the `table` ancestor of `rp`. Returns
/// `(table_depth, row_index, col_index)`.
fn find_table(rp: &ResolvedPos) -> Option<(usize, usize, usize)> {
    for d in 1..=rp.depth() {
        if rp.node(d).node_type().name() == "table" {
            // The row is node(d+1)'s container: rp.index(d) is the row
            // index within the table, rp.index(d+1) the cell within the row.
            if d + 1 > rp.depth() {
                return None;
            }
            return Some((d, rp.index(d), rp.index(d + 1)));
        }
    }
    None
}

/// Absolute caret position inside cell `(row, col)` of `table`, where the
/// table node begins at `tstart`. Row/col are clamped to the grid, and the
/// caret lands at the start of the cell's first block — so chained
/// structural commands always re-resolve into the table.
fn cell_caret_pos(table: &Node, tstart: usize, row: usize, col: usize) -> usize {
    let row = row.min(table.child_count().saturating_sub(1));
    let mut pos = tstart + 1; // inside table, before the first row
    for (ri, r) in table.content().iter().enumerate() {
        if ri == row {
            let col = col.min(r.child_count().saturating_sub(1));
            let mut cpos = pos + 1; // inside row, before the first cell
            for (ci, c) in r.content().iter().enumerate() {
                if ci == col {
                    return cpos + 2; // inside cell, into its first block
                }
                cpos += c.node_size();
            }
            return cpos + 2;
        }
        pos += r.node_size();
    }
    tstart + 4
}

/// Replace the table at `table_depth` (the one enclosing `rp`) with
/// `new_table`, or delete the range if `new_table` is `None`. When a new
/// table is supplied, the caret is moved into its `target` cell so the
/// next structural command still resolves into the table.
fn replace_table(
    state: &taino_edit_core::EditorState,
    rp: &ResolvedPos,
    table_depth: usize,
    new_table: Option<Node>,
    target: Option<(usize, usize)>,
    dispatch: Option<&mut taino_edit_core::Dispatch<'_>>,
) {
    let start = rp.before(table_depth);
    let end = rp.after(table_depth);
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        match new_table {
            Some(t) => {
                let caret = target.map(|(r, c)| cell_caret_pos(&t, start, r, c));
                let slice = Slice::new(Fragment::from_node(t), 0, 0);
                if tx
                    .transform()
                    .replace(start, end, slice, state.schema())
                    .is_ok()
                {
                    if let Some(p) = caret {
                        tx.set_selection(Selection::caret(p));
                    }
                    d(tx);
                }
            }
            None => {
                if tx.transform().delete(start, end, state.schema()).is_ok() {
                    tx.set_selection(Selection::caret(start.saturating_sub(1)));
                    d(tx);
                }
            }
        }
    }
}

// ---- commands ------------------------------------------------------------

/// Insert an empty `rows`×`cols` table as a new block after the block
/// enclosing the caret. The caret moves into the first cell.
pub fn insert_table(rows: usize, cols: usize) -> Command {
    Box::new(move |state, dispatch| {
        if rows == 0 || cols == 0 || state.schema().node_type("table").is_none() {
            return false;
        }
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        if rp.depth() == 0 {
            return false;
        }
        let insert_pos = rp.after(1);
        let Some(table) = build_table(state.schema(), rows, cols) else {
            return false;
        };
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let slice = Slice::new(Fragment::from_node(table), 0, 0);
            if tx
                .transform()
                .replace(insert_pos, insert_pos, slice, state.schema())
                .is_ok()
            {
                // table(+1) row(+1) cell(+1) paragraph(+1) → first inner pos.
                tx.set_selection(Selection::caret(insert_pos + 4));
                d(tx);
            }
        }
        true
    })
}

fn add_column(after: bool) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, row, col)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let insert_at = if after { col + 1 } else { col };
        let mut new_rows = Vec::with_capacity(table.child_count());
        for row in table.content().iter() {
            let mut cells = row.content().children().to_vec();
            let at = insert_at.min(cells.len());
            let Some(cell) = empty_cell(state.schema()) else {
                return false;
            };
            cells.insert(at, cell);
            let Ok(new_row) = state
                .schema()
                .node("table_row", row.attrs().clone(), cells, vec![])
            else {
                return false;
            };
            new_rows.push(new_row);
        }
        let Ok(new_table) = state
            .schema()
            .node("table", table.attrs().clone(), new_rows, vec![])
        else {
            return false;
        };
        let target = if after { (row, col) } else { (row, col + 1) };
        replace_table(state, &rp, td, Some(new_table), Some(target), dispatch);
        true
    })
}

/// Insert a column before the caret's column.
pub fn add_column_before() -> Command {
    add_column(false)
}
/// Insert a column after the caret's column.
pub fn add_column_after() -> Command {
    add_column(true)
}

/// Delete the caret's column. Deletes the whole table if it was the last
/// column.
pub fn delete_column() -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, row, col)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let n_cols = table.child(0).child_count();
        if n_cols <= 1 {
            replace_table(state, &rp, td, None, None, dispatch);
            return true;
        }
        let mut new_rows = Vec::with_capacity(table.child_count());
        for row in table.content().iter() {
            let mut cells = row.content().children().to_vec();
            if col < cells.len() {
                cells.remove(col);
            }
            let Ok(new_row) = state
                .schema()
                .node("table_row", row.attrs().clone(), cells, vec![])
            else {
                return false;
            };
            new_rows.push(new_row);
        }
        let Ok(new_table) = state
            .schema()
            .node("table", table.attrs().clone(), new_rows, vec![])
        else {
            return false;
        };
        // After removing column `col`, clamp the caret to the new width.
        replace_table(state, &rp, td, Some(new_table), Some((row, col)), dispatch);
        true
    })
}

fn add_row(after: bool) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, row, col)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let n_cols = table.child(0).child_count();
        let mut cells = Vec::with_capacity(n_cols);
        for _ in 0..n_cols {
            let Some(c) = empty_cell(state.schema()) else {
                return false;
            };
            cells.push(c);
        }
        let Ok(new_row) = state
            .schema()
            .node("table_row", Default::default(), cells, vec![])
        else {
            return false;
        };
        let mut rows = table.content().children().to_vec();
        let at = if after { row + 1 } else { row };
        let at = at.min(rows.len());
        rows.insert(at, new_row);
        let Ok(new_table) = state
            .schema()
            .node("table", table.attrs().clone(), rows, vec![])
        else {
            return false;
        };
        replace_table(state, &rp, td, Some(new_table), Some((at, col)), dispatch);
        true
    })
}

/// Insert a row before the caret's row.
pub fn add_row_before() -> Command {
    add_row(false)
}
/// Insert a row after the caret's row.
pub fn add_row_after() -> Command {
    add_row(true)
}

/// Delete the caret's row. Deletes the whole table if it was the last row.
pub fn delete_row() -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, row, col)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        if table.child_count() <= 1 {
            replace_table(state, &rp, td, None, None, dispatch);
            return true;
        }
        let mut rows = table.content().children().to_vec();
        if row < rows.len() {
            rows.remove(row);
        }
        let Ok(new_table) = state
            .schema()
            .node("table", table.attrs().clone(), rows, vec![])
        else {
            return false;
        };
        replace_table(state, &rp, td, Some(new_table), Some((row, col)), dispatch);
        true
    })
}

/// Delete the whole table containing the caret.
pub fn delete_table() -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, _row, _col)) = find_table(&rp) else {
            return false;
        };
        replace_table(state, &rp, td, None, None, dispatch);
        true
    })
}

// ---- header toggling -----------------------------------------------------

/// Set the `header` attr on a cell, returning a rebuilt cell node.
fn with_header(schema: &Schema, cell: &Node, header: bool) -> Option<Node> {
    let mut attrs = cell.attrs().clone();
    attrs.insert("header".into(), AttrValue::from(header));
    schema
        .node(
            "table_cell",
            attrs,
            cell.content().children().to_vec(),
            cell.marks().to_vec(),
        )
        .ok()
}

/// Whether every cell in `cells` is already a header.
fn all_header(cells: &[Node]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|c| {
            c.attrs()
                .get("header")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
}

/// Which cells a header-toggle command targets.
enum HeaderScope {
    Row,
    Column,
    Cell,
}

fn toggle_header(scope: HeaderScope) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, row, col)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);

        // Decide the new header value by inverting the current state of the
        // targeted cells.
        let targeted: Vec<&Node> = match scope {
            HeaderScope::Row => table.child(row).content().children().iter().collect(),
            HeaderScope::Column => table
                .content()
                .iter()
                .filter_map(|r| r.content().children().get(col))
                .collect(),
            HeaderScope::Cell => table
                .child(row)
                .content()
                .children()
                .get(col)
                .into_iter()
                .collect(),
        };
        if targeted.is_empty() {
            return false;
        }
        let make_header = !all_header(&targeted.iter().map(|c| (*c).clone()).collect::<Vec<_>>());

        let schema = state.schema();
        let mut new_rows = Vec::with_capacity(table.child_count());
        for (ri, r) in table.content().iter().enumerate() {
            let mut cells = r.content().children().to_vec();
            for (ci, cell) in cells.iter_mut().enumerate() {
                let hit = match scope {
                    HeaderScope::Row => ri == row,
                    HeaderScope::Column => ci == col,
                    HeaderScope::Cell => ri == row && ci == col,
                };
                if hit {
                    let Some(updated) = with_header(schema, cell, make_header) else {
                        return false;
                    };
                    *cell = updated;
                }
            }
            let Ok(new_row) = schema.node("table_row", r.attrs().clone(), cells, vec![]) else {
                return false;
            };
            new_rows.push(new_row);
        }
        let Ok(new_table) = schema.node("table", table.attrs().clone(), new_rows, vec![]) else {
            return false;
        };
        replace_table(state, &rp, td, Some(new_table), Some((row, col)), dispatch);
        true
    })
}

/// Toggle the `<th>`/`<td>` state of every cell in the caret's row.
pub fn toggle_header_row() -> Command {
    toggle_header(HeaderScope::Row)
}
/// Toggle the `<th>`/`<td>` state of every cell in the caret's column.
pub fn toggle_header_column() -> Command {
    toggle_header(HeaderScope::Column)
}
/// Toggle the `<th>`/`<td>` state of just the caret's cell.
pub fn toggle_header_cell() -> Command {
    toggle_header(HeaderScope::Cell)
}
