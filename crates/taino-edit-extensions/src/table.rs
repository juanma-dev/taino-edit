//! Tables — `table` / `table_row` / `table_cell` nodes plus structural
//! commands (insert table, add/delete rows and columns, delete table).
//!
//! v0.3 ships a **structural** table MVP: the schema, an HTML round-trip
//! (`<table><tr><td>…`), and commands that operate on the cell containing
//! the caret. Cell-range selection, merge/split, column resizing, header
//! toggling and Tab cell-navigation are deliberately deferred — they need
//! a dedicated `CellSelection` and DOM-level drag handling that are their
//! own body of work.
//!
//! All structural commands rebuild the enclosing `table` node and replace
//! it wholesale. Tables are small, so the O(table size) rebuild is cheap
//! and keeps the position arithmetic obviously correct.

use taino_edit_core::{
    Command, DomSpec, Fragment, Node, NodeSpec, ParseRule, ResolvedPos, Schema, Selection, Slice,
};

use crate::{Extension, SchemaAdditions};

/// The table extension. Adds `table`, `table_row` and `table_cell` nodes.
/// No default keymap — the structural commands are exported for a host
/// toolbar to wire (cell navigation would collide with the Lists `Tab`
/// binding, so it's left to the host).
pub struct Table;

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
                        to_dom: Some(|_| DomSpec::element("td")),
                        parse_dom: vec![ParseRule::tag("td"), ParseRule::tag("th")],
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
