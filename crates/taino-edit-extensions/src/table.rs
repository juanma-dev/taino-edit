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
    a.insert(
        "colwidth".to_string(),
        AttrSpec {
            default: Some(AttrValue::Null),
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
    if let Some(w) = n.attrs().get("colwidth").and_then(|v| v.as_u64()) {
        spec = spec.attr("style", format!("width: {w}px"));
    }
    spec
}

/// Extract a `width: Npx` declaration from a `style` attribute value.
fn parse_style_width(style: &str) -> Option<u64> {
    let lower = style.to_ascii_lowercase();
    let idx = lower.find("width")?;
    let after = lower[idx + "width".len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u64>().ok()
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
    match el.attr("style").and_then(parse_style_width) {
        Some(w) => a.insert("colwidth".into(), AttrValue::from(w)),
        None => a.insert("colwidth".into(), AttrValue::Null),
    };
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

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        // `Tab` / `Shift-Tab` move between cells. `build_keymap_with`
        // chains these in front of any existing binding (e.g. the Lists
        // sink/lift), and each command is a no-op outside a table, so the
        // two cooperate.
        vec![
            ("Tab".to_string(), go_to_next_cell()),
            ("Shift-Tab".to_string(), go_to_prev_cell()),
        ]
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

fn colspan_of(cell: &Node) -> usize {
    cell.attrs()
        .get("colspan")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as usize
}
fn rowspan_of(cell: &Node) -> usize {
    cell.attrs()
        .get("rowspan")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as usize
}

/// A resolved logical grid for a table, accounting for colspan/rowspan.
/// `cells[r * width + c]` is the *document index* `(row_index, cell_index)`
/// of the cell that covers logical position `(r, c)`.
struct TableMap {
    width: usize,
    height: usize,
    cells: Vec<(usize, usize)>,
}

impl TableMap {
    fn of(table: &Node) -> TableMap {
        let height = table.child_count();
        // Logical width = sum of colspans across row 0 (row 0 has no
        // incoming rowspans in a well-formed table).
        let width = if height == 0 {
            0
        } else {
            table
                .child(0)
                .content()
                .iter()
                .map(colspan_of)
                .sum::<usize>()
        };
        let mut cells = vec![(usize::MAX, usize::MAX); width * height];
        for (r, row) in table.content().iter().enumerate() {
            let mut col = 0;
            for (ci, cell) in row.content().iter().enumerate() {
                // Skip logical columns already filled by a rowspan above.
                while col < width && cells[r * width + col] != (usize::MAX, usize::MAX) {
                    col += 1;
                }
                let cs = colspan_of(cell);
                let rs = rowspan_of(cell);
                for dr in 0..rs {
                    for dc in 0..cs {
                        let (rr, cc) = (r + dr, col + dc);
                        if rr < height && cc < width {
                            cells[rr * width + cc] = (r, ci);
                        }
                    }
                }
                col += cs;
            }
        }
        TableMap {
            width,
            height,
            cells,
        }
    }

    /// The `(row_index, cell_index)` covering logical `(r, c)`.
    fn at(&self, r: usize, c: usize) -> Option<(usize, usize)> {
        if r < self.height && c < self.width {
            let v = self.cells[r * self.width + c];
            (v != (usize::MAX, usize::MAX)).then_some(v)
        } else {
            None
        }
    }

    /// The top-left logical coordinate of the cell at document index
    /// `(row_index, cell_index)`.
    fn logical_of(&self, doc_cell: (usize, usize)) -> Option<(usize, usize)> {
        for r in 0..self.height {
            for c in 0..self.width {
                if self.cells[r * self.width + c] == doc_cell {
                    return Some((r, c));
                }
            }
        }
        None
    }
}

/// Absolute position directly before cell at document index
/// `(row_index, cell_index)` of `table` starting at `tstart`.
fn cell_before_abs(table: &Node, tstart: usize, row: usize, cell_idx: usize) -> usize {
    let mut pos = tstart + 1; // before first row
    for (ri, r) in table.content().iter().enumerate() {
        if ri == row {
            let mut cpos = pos + 1; // inside row, before first cell
            for (ci, c) in r.content().iter().enumerate() {
                if ci == cell_idx {
                    return cpos;
                }
                cpos += c.node_size();
            }
            return cpos;
        }
        pos += r.node_size();
    }
    tstart + 1
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

// ---- cell navigation -----------------------------------------------------

/// Move the caret to the start of cell `(r, c)` of the table that begins
/// at `tstart`.
fn dispatch_caret_to(
    state: &taino_edit_core::EditorState,
    table: &Node,
    tstart: usize,
    r: usize,
    c: usize,
    dispatch: Option<&mut taino_edit_core::Dispatch<'_>>,
) {
    if let Some(d) = dispatch {
        let pos = cell_caret_pos(table, tstart, r, c);
        let mut tx = state.tr();
        tx.set_selection(Selection::caret(pos));
        d(tx);
    }
}

fn go_to_cell(forward: bool) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, row, col)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let n_rows = table.child_count();
        let cur_cols = table.child(row).child_count();
        let tstart = rp.before(td);

        if forward {
            if col + 1 < cur_cols {
                dispatch_caret_to(state, table, tstart, row, col + 1, dispatch);
                return true;
            }
            if row + 1 < n_rows {
                dispatch_caret_to(state, table, tstart, row + 1, 0, dispatch);
                return true;
            }
            // Past the last cell: append a fresh row and land in it.
            let n_cols = cur_cols;
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
            rows.push(new_row);
            let Ok(new_table) = state
                .schema()
                .node("table", table.attrs().clone(), rows, vec![])
            else {
                return false;
            };
            replace_table(state, &rp, td, Some(new_table), Some((n_rows, 0)), dispatch);
            true
        } else {
            if col > 0 {
                dispatch_caret_to(state, table, tstart, row, col - 1, dispatch);
                return true;
            }
            if row > 0 {
                let prev_cols = table.child(row - 1).child_count();
                dispatch_caret_to(state, table, tstart, row - 1, prev_cols - 1, dispatch);
                return true;
            }
            // Already at the very first cell — let the binding fall through.
            false
        }
    })
}

/// Move the caret to the next cell (left-to-right, top-to-bottom). Past the
/// last cell it appends a new row. Bound to `Tab`.
pub fn go_to_next_cell() -> Command {
    go_to_cell(true)
}

/// Move the caret to the previous cell. A no-op at the first cell (so a
/// chained binding such as the Lists lift can take over). Bound to
/// `Shift-Tab`.
pub fn go_to_prev_cell() -> Command {
    go_to_cell(false)
}

// ---- cell-range selection, merge & split ---------------------------------

/// Set a [`Selection::Cell`] covering the rectangle between logical cells
/// `anchor` and `head` (each `(row, col)`) of the table containing the
/// caret. A host wires this to mouse drag-across-cells; tests use it to
/// build a cell selection directly.
pub fn select_cell_range(anchor: (usize, usize), head: (usize, usize)) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, _, _)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let tstart = rp.before(td);
        let map = TableMap::of(table);
        let Some(a) = map.at(anchor.0, anchor.1) else {
            return false;
        };
        let Some(h) = map.at(head.0, head.1) else {
            return false;
        };
        let anchor_pos = cell_before_abs(table, tstart, a.0, a.1);
        let head_pos = cell_before_abs(table, tstart, h.0, h.1);
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            tx.set_selection(Selection::Cell {
                anchor: anchor_pos,
                head: head_pos,
            });
            d(tx);
        }
        true
    })
}

/// Merge the cells covered by the current [`Selection::Cell`] into one,
/// with the matching colspan/rowspan and the concatenated content of all
/// merged cells. A no-op unless a multi-cell range is selected.
pub fn merge_cells() -> Command {
    Box::new(move |state, dispatch| {
        let Selection::Cell { anchor, head } = state.selection() else {
            return false;
        };
        let Ok(rp) = ResolvedPos::resolve(state.doc(), anchor.min(head) + 1) else {
            return false;
        };
        let Some((td, _, _)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let tstart = rp.before(td);
        let map = TableMap::of(table);

        // Logical coordinates of the anchor & head cells.
        let cell_at_pos = |p: usize| -> Option<(usize, usize)> {
            // Resolve which document cell starts at abs position p, then map.
            for (ri, row) in table.content().iter().enumerate() {
                for (ci, _) in row.content().iter().enumerate() {
                    if cell_before_abs(table, tstart, ri, ci) == p {
                        return map.logical_of((ri, ci));
                    }
                }
            }
            None
        };
        let (Some((ar, ac)), Some((hr, hc))) = (cell_at_pos(anchor), cell_at_pos(head)) else {
            return false;
        };
        let (r0, r1) = (ar.min(hr), ar.max(hr));
        let (c0, c1) = (ac.min(hc), ac.max(hc));
        if r0 == r1 && c0 == c1 {
            return false; // single cell — nothing to merge
        }

        // Document cells inside the rectangle, in reading order.
        let mut covered: Vec<(usize, usize)> = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                if let Some(dc) = map.at(r, c) {
                    if !covered.contains(&dc) {
                        covered.push(dc);
                    }
                }
            }
        }
        let top_left = match map.at(r0, c0) {
            Some(tl) => tl,
            None => return false,
        };

        // Merge all covered cells' block content into the top-left cell and
        // give it the new spans.
        let schema = state.schema();
        let mut merged_blocks: Vec<Node> = Vec::new();
        for (ri, ci) in &covered {
            merged_blocks.extend(table.child(*ri).child(*ci).content().children().to_vec());
        }
        let mut attrs = table.child(top_left.0).child(top_left.1).attrs().clone();
        attrs.insert("colspan".into(), AttrValue::from((c1 - c0 + 1) as u64));
        attrs.insert("rowspan".into(), AttrValue::from((r1 - r0 + 1) as u64));
        let Ok(merged_cell) = schema.node("table_cell", attrs, merged_blocks, vec![]) else {
            return false;
        };

        // Rebuild rows, dropping covered cells except the top-left (replaced
        // by the merged cell).
        let mut new_rows = Vec::with_capacity(table.child_count());
        for (ri, row) in table.content().iter().enumerate() {
            let mut cells = Vec::new();
            for (ci, cell) in row.content().iter().enumerate() {
                if (ri, ci) == top_left {
                    cells.push(merged_cell.clone());
                } else if covered.contains(&(ri, ci)) {
                    // dropped
                } else {
                    cells.push(cell.clone());
                }
            }
            // A row may legitimately become empty only if the whole row was
            // inside the rectangle and not the top-left row; skip such rows.
            if cells.is_empty() {
                continue;
            }
            let Ok(new_row) = schema.node("table_row", row.attrs().clone(), cells, vec![]) else {
                return false;
            };
            new_rows.push(new_row);
        }
        let Ok(new_table) = schema.node("table", table.attrs().clone(), new_rows, vec![]) else {
            return false;
        };
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let start = rp.before(td);
            let end = rp.after(td);
            let slice = Slice::new(Fragment::from_node(new_table), 0, 0);
            if tx
                .transform()
                .replace(start, end, slice, state.schema())
                .is_ok()
            {
                // Caret into the merged (top-left) cell.
                tx.set_selection(Selection::caret(start + 4));
                d(tx);
            }
        }
        true
    })
}

/// Split the cell containing the caret (with colspan and/or rowspan > 1)
/// back into 1×1 cells. The original content stays in the top-left; the
/// new cells are empty. A no-op on an unspanned cell.
pub fn split_cell() -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, row, col)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let cell = table.child(row).child(col);
        let cs = colspan_of(cell);
        let rs = rowspan_of(cell);
        if cs == 1 && rs == 1 {
            return false;
        }
        let schema = state.schema();
        let map = TableMap::of(table);
        let Some((lr, lc)) = map.logical_of((row, col)) else {
            return false;
        };

        // The unspanned (1×1) version of the original cell, keeping content.
        let mut base_attrs = cell.attrs().clone();
        base_attrs.insert("colspan".into(), AttrValue::from(1u64));
        base_attrs.insert("rowspan".into(), AttrValue::from(1u64));
        let Ok(kept) = schema.node(
            "table_cell",
            base_attrs,
            cell.content().children().to_vec(),
            vec![],
        ) else {
            return false;
        };

        // Rebuild every row, inserting fresh cells for the freed logical
        // columns. We work per logical row in the cell's vertical span.
        let mut new_rows = Vec::with_capacity(table.child_count());
        for (ri, r) in table.content().iter().enumerate() {
            // Cells of this row that are NOT the spanned cell stay; we then
            // splice the split cells into the right logical span.
            if !(lr..lr + rs).contains(&ri) {
                new_rows.push(r.clone());
                continue;
            }
            // Rebuild this row: copy existing cells, replacing the spanned
            // one (only present in its top row) and adding 1×1 cells across
            // its logical columns.
            let mut cells: Vec<Node> = Vec::new();
            for (ci, c) in r.content().iter().enumerate() {
                if ri == row && ci == col {
                    // Top-left: emit the kept cell plus fillers for the rest
                    // of this logical row's span.
                    cells.push(kept.clone());
                    for _ in 1..cs {
                        let Some(e) = empty_cell(schema) else {
                            return false;
                        };
                        cells.push(e);
                    }
                } else {
                    cells.push(c.clone());
                }
            }
            // For rows below the top (rowspan), the spanned cell wasn't a
            // member, so add a full run of fillers at the freed columns.
            if ri != row {
                let mut fillers = Vec::new();
                for _ in 0..cs {
                    let Some(e) = empty_cell(schema) else {
                        return false;
                    };
                    fillers.push(e);
                }
                // Insert fillers at logical column lc (best-effort: append at
                // the position matching lc among existing cells).
                let insert_at = lc.min(cells.len());
                for (k, f) in fillers.into_iter().enumerate() {
                    cells.insert(insert_at + k, f);
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
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let start = rp.before(td);
            let end = rp.after(td);
            let caret = cell_caret_pos(&new_table, start, lr, lc);
            let slice = Slice::new(Fragment::from_node(new_table), 0, 0);
            if tx
                .transform()
                .replace(start, end, slice, state.schema())
                .is_ok()
            {
                tx.set_selection(Selection::caret(caret));
                d(tx);
            }
        }
        true
    })
}

// ---- column resizing -----------------------------------------------------

/// Set the pixel width of logical column `col` of the table at the caret —
/// the value behind a drag-to-resize grip. `width` is clamped to a small
/// minimum. The width is stored as a `colwidth` attr on every cell covering
/// that column and rendered as `style="width: …px"`.
pub fn set_column_width(col: usize, width: u64) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, caret_row, caret_col)) = find_table(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let map = TableMap::of(table);
        if col >= map.width {
            return false;
        }
        let w = width.max(24); // sane minimum column width
                               // Document cells covering logical column `col` (one per logical row).
        let mut targets: Vec<(usize, usize)> = Vec::new();
        for r in 0..map.height {
            if let Some(dc) = map.at(r, col) {
                if !targets.contains(&dc) {
                    targets.push(dc);
                }
            }
        }
        let schema = state.schema();
        let mut new_rows = Vec::with_capacity(table.child_count());
        for (ri, row) in table.content().iter().enumerate() {
            let mut cells = row.content().children().to_vec();
            for (ci, cell) in cells.iter_mut().enumerate() {
                if targets.contains(&(ri, ci)) {
                    let mut attrs = cell.attrs().clone();
                    attrs.insert("colwidth".into(), AttrValue::from(w));
                    let Ok(updated) = schema.node(
                        "table_cell",
                        attrs,
                        cell.content().children().to_vec(),
                        cell.marks().to_vec(),
                    ) else {
                        return false;
                    };
                    *cell = updated;
                }
            }
            let Ok(new_row) = schema.node("table_row", row.attrs().clone(), cells, vec![]) else {
                return false;
            };
            new_rows.push(new_row);
        }
        let Ok(new_table) = schema.node("table", table.attrs().clone(), new_rows, vec![]) else {
            return false;
        };
        // Attr-only change, but it's a full-table replace, so keep the caret
        // in its cell explicitly.
        replace_table(
            state,
            &rp,
            td,
            Some(new_table),
            Some((caret_row, caret_col)),
            dispatch,
        );
        true
    })
}
