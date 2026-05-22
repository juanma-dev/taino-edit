//! Tables — `table` / `table_row` / `table_cell` nodes plus the full
//! command vocabulary a table-aware editor needs.
//!
//! Cells carry `colspan` / `rowspan` / `header` / `colwidth` attrs and
//! round-trip as `<table><tr><td>` / `<th>`. Every structural command is
//! **span-aware**: they decompose the table into a list of [`Placement`]s
//! (cells with logical-grid coordinates + spans), transform that list, and
//! re-render through [`render_placements`], which compacts rows/columns
//! that become fully span-covered and recomputes every span against the
//! compacted grid. The result is always a well-formed, rectangular table —
//! merging, splitting, adding and deleting rows/columns can never leave an
//! orphan `rowspan`/`colspan` or an empty `<tr>`.
//!
//! Commands: `insert_table`, add/delete rows and columns, `delete_table`,
//! `toggle_header_*`, `go_to_next_cell`/`go_to_prev_cell` (Tab/Shift-Tab),
//! `select_cell_range` + `merge_cells` + `split_cell`, and
//! `set_column_width`.

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

/// A cell placed on the logical grid: its top-left logical coordinate, its
/// span, and the cell node. Structural table operations are expressed as
/// transformations of a `Vec<Placement>`, then re-rendered to rows. This
/// keeps colspan/rowspan correct under every edit.
#[derive(Clone)]
struct Placement {
    row: usize,
    col: usize,
    colspan: usize,
    rowspan: usize,
    cell: Node,
}

/// Decompose a table into logical-grid placements (one per origin cell).
fn placements_of(table: &Node) -> Vec<Placement> {
    let map = TableMap::of(table);
    let mut out = Vec::new();
    let mut seen: Vec<(usize, usize)> = Vec::new();
    for r in 0..map.height {
        for c in 0..map.width {
            if let Some(doc) = map.at(r, c) {
                if seen.contains(&doc) {
                    continue;
                }
                seen.push(doc);
                let cell = table.child(doc.0).child(doc.1).clone();
                out.push(Placement {
                    row: r,
                    col: c,
                    colspan: colspan_of(&cell),
                    rowspan: rowspan_of(&cell),
                    cell,
                });
            }
        }
    }
    out
}

/// Re-render placements into a `table` node. Rows/columns that became
/// entirely covered by spans (no cell originates in them) are dropped and
/// the crossing spans are reduced accordingly — so the result is always a
/// well-formed, rectangular table with no orphan colspan/rowspan. Returns
/// `None` if there are no cells left (caller deletes the table).
fn render_placements(
    schema: &Schema,
    placements: &[Placement],
    table_attrs: Attrs,
) -> Option<Node> {
    if placements.is_empty() {
        return None;
    }
    let raw_h = placements
        .iter()
        .map(|p| p.row + p.rowspan)
        .max()
        .unwrap_or(0);
    let raw_w = placements
        .iter()
        .map(|p| p.col + p.colspan)
        .max()
        .unwrap_or(0);

    // "Real" rows/cols are those some cell originates in; any other
    // row/col is pure span-coverage and is dropped.
    let real_rows: Vec<usize> = (0..raw_h)
        .filter(|&r| placements.iter().any(|p| p.row == r))
        .collect();
    let real_cols: Vec<usize> = (0..raw_w)
        .filter(|&c| placements.iter().any(|p| p.col == c))
        .collect();
    if real_rows.is_empty() || real_cols.is_empty() {
        return None;
    }
    let row_compact = |raw: usize| real_rows.iter().position(|&x| x == raw);
    let col_compact = |raw: usize| real_cols.iter().position(|&x| x == raw);

    // Bucket placements by compacted row.
    let mut rows: Vec<Vec<(usize, Node)>> = vec![Vec::new(); real_rows.len()];
    for p in placements {
        let Some(nr) = row_compact(p.row) else {
            continue;
        };
        let Some(nc) = col_compact(p.col) else {
            continue;
        };
        // Recompute spans against the compacted grid.
        let new_rowspan = real_rows
            .iter()
            .filter(|&&r| r >= p.row && r < p.row + p.rowspan)
            .count()
            .max(1);
        let new_colspan = real_cols
            .iter()
            .filter(|&&c| c >= p.col && c < p.col + p.colspan)
            .count()
            .max(1);
        let mut attrs = p.cell.attrs().clone();
        attrs.insert("colspan".into(), AttrValue::from(new_colspan as u64));
        attrs.insert("rowspan".into(), AttrValue::from(new_rowspan as u64));
        let cell = schema
            .node(
                "table_cell",
                attrs,
                p.cell.content().children().to_vec(),
                p.cell.marks().to_vec(),
            )
            .ok()?;
        rows[nr].push((nc, cell));
    }

    let mut row_nodes = Vec::with_capacity(rows.len());
    for mut cells in rows {
        if cells.is_empty() {
            // Should not happen (real rows have an origin), but never emit
            // an empty <tr>.
            return None;
        }
        cells.sort_by_key(|(c, _)| *c);
        let ordered: Vec<Node> = cells.into_iter().map(|(_, n)| n).collect();
        row_nodes.push(
            schema
                .node("table_row", Default::default(), ordered, vec![])
                .ok()?,
        );
    }
    schema.node("table", table_attrs, row_nodes, vec![]).ok()
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

/// Replace the table with `new_table` and place the caret in its logical
/// `(row, col)` cell (clamped to the new grid).
fn replace_table_logical(
    state: &taino_edit_core::EditorState,
    rp: &ResolvedPos,
    table_depth: usize,
    new_table: Node,
    logical_target: (usize, usize),
    dispatch: Option<&mut taino_edit_core::Dispatch<'_>>,
) {
    let start = rp.before(table_depth);
    let end = rp.after(table_depth);
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        let map = TableMap::of(&new_table);
        let lr = logical_target.0.min(map.height.saturating_sub(1));
        let lc = logical_target.1.min(map.width.saturating_sub(1));
        let caret = map
            .at(lr, lc)
            .map(|(ri, ci)| cell_caret_pos(&new_table, start, ri, ci))
            .unwrap_or(start + 4);
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

/// The caret's logical position within its table: `(table_depth,
/// logical_row, logical_col)`.
fn caret_logical(rp: &ResolvedPos) -> Option<(usize, usize, usize)> {
    let (td, row_idx, cell_idx) = find_table(rp)?;
    let map = TableMap::of(rp.node(td));
    let (_, lcol) = map.logical_of((row_idx, cell_idx))?;
    Some((td, row_idx, lcol))
}

fn add_column(after: bool) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, lrow, lcol)) = caret_logical(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let map = TableMap::of(table);
        let at = if after { lcol + 1 } else { lcol };
        let mut placements = placements_of(table);
        // Shift / grow existing cells around the insertion column.
        for p in placements.iter_mut() {
            if p.col >= at {
                p.col += 1;
            } else if p.col + p.colspan > at {
                p.colspan += 1; // straddles the boundary → widen
            }
        }
        // Add fresh 1×1 cells in rows where the new column is uncovered.
        for r in 0..map.height {
            if !cell_covered(&placements, r, at) {
                let Some(cell) = empty_cell(state.schema()) else {
                    return false;
                };
                placements.push(Placement {
                    row: r,
                    col: at,
                    colspan: 1,
                    rowspan: 1,
                    cell,
                });
            }
        }
        let Some(new_table) = render_placements(state.schema(), &placements, table.attrs().clone())
        else {
            return false;
        };
        let target = (lrow, if after { lcol } else { lcol + 1 });
        replace_table_logical(state, &rp, td, new_table, target, dispatch);
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
        let Some((td, lrow, lcol)) = caret_logical(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let mut placements: Vec<Placement> = Vec::new();
        for p in placements_of(table) {
            if p.col <= lcol && lcol < p.col + p.colspan {
                // Covers the doomed column.
                if p.colspan > 1 {
                    let mut q = p.clone();
                    q.colspan -= 1; // shrink; origin col stays (boundary)
                    placements.push(q);
                }
                // colspan == 1 → drop the cell entirely
            } else {
                let mut q = p.clone();
                if q.col > lcol {
                    q.col -= 1;
                }
                placements.push(q);
            }
        }
        match render_placements(state.schema(), &placements, table.attrs().clone()) {
            Some(new_table) => {
                replace_table_logical(state, &rp, td, new_table, (lrow, lcol), dispatch)
            }
            None => replace_table(state, &rp, td, None, None, dispatch),
        }
        true
    })
}

/// Whether logical cell `(r, c)` is covered by any placement.
fn cell_covered(placements: &[Placement], r: usize, c: usize) -> bool {
    placements
        .iter()
        .any(|p| r >= p.row && r < p.row + p.rowspan && c >= p.col && c < p.col + p.colspan)
}

fn add_row(after: bool) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((td, lrow, lcol)) = caret_logical(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let map = TableMap::of(table);
        let at = if after { lrow + 1 } else { lrow };
        let mut placements = placements_of(table);
        // Shift / grow existing cells around the insertion row.
        for p in placements.iter_mut() {
            if p.row >= at {
                p.row += 1;
            } else if p.row + p.rowspan > at {
                p.rowspan += 1; // a rowspan straddles the boundary → grow it
            }
        }
        // Add fresh 1×1 cells in columns where the new row is uncovered.
        for c in 0..map.width {
            if !cell_covered(&placements, at, c) {
                let Some(cell) = empty_cell(state.schema()) else {
                    return false;
                };
                placements.push(Placement {
                    row: at,
                    col: c,
                    colspan: 1,
                    rowspan: 1,
                    cell,
                });
            }
        }
        let Some(new_table) = render_placements(state.schema(), &placements, table.attrs().clone())
        else {
            return false;
        };
        let target = (at, lcol);
        replace_table_logical(state, &rp, td, new_table, target, dispatch);
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
        let Some((td, lrow, lcol)) = caret_logical(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let mut placements: Vec<Placement> = Vec::new();
        for p in placements_of(table) {
            if p.row <= lrow && lrow < p.row + p.rowspan {
                // Covers the doomed row.
                if p.rowspan > 1 {
                    let mut q = p.clone();
                    q.rowspan -= 1; // shrink; origin row stays (boundary)
                    placements.push(q);
                }
                // rowspan == 1 → drop the cell entirely
            } else {
                let mut q = p.clone();
                if q.row > lrow {
                    q.row -= 1;
                }
                placements.push(q);
            }
        }
        match render_placements(state.schema(), &placements, table.attrs().clone()) {
            Some(new_table) => {
                replace_table_logical(state, &rp, td, new_table, (lrow, lcol), dispatch)
            }
            None => replace_table(state, &rp, td, None, None, dispatch),
        }
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
            // Past the last cell: append a fresh row (logical-width many
            // empty cells, span-aware) and land in its first cell.
            let logical_w = TableMap::of(table).width;
            let mut placements = placements_of(table);
            for c in 0..logical_w {
                let Some(cell) = empty_cell(state.schema()) else {
                    return false;
                };
                placements.push(Placement {
                    row: n_rows,
                    col: c,
                    colspan: 1,
                    rowspan: 1,
                    cell,
                });
            }
            let Some(new_table) =
                render_placements(state.schema(), &placements, table.attrs().clone())
            else {
                return false;
            };
            replace_table_logical(state, &rp, td, new_table, (n_rows, 0), dispatch);
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

// ---- pointer-plugin helpers ----------------------------------------------

/// Where a document position sits within a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellAt {
    /// Position directly before the enclosing `table` node.
    pub table_pos: usize,
    /// Position directly before the enclosing cell.
    pub cell_pos: usize,
    /// Logical row of the cell.
    pub row: usize,
    /// Logical column of the cell.
    pub col: usize,
}

/// Resolve the table cell containing document position `pos`, if any. Used
/// by pointer-driven UI (cell drag-select, column resize) to turn a mapped
/// point into a cell.
pub fn cell_at(doc: &Node, pos: usize) -> Option<CellAt> {
    let rp = ResolvedPos::resolve(doc, pos).ok()?;
    let (td, row_idx, cell_idx) = find_table(&rp)?;
    let table = rp.node(td);
    let table_pos = rp.before(td);
    let map = TableMap::of(table);
    let (row, col) = map.logical_of((row_idx, cell_idx))?;
    Some(CellAt {
        table_pos,
        cell_pos: cell_before_abs(table, table_pos, row_idx, cell_idx),
        row,
        col,
    })
}

/// The cell-before positions covered by a [`Selection::Cell`] rectangle
/// (expanded to whole spans, as a merge would). Empty for any other
/// selection or a non-table position. Used to highlight selected cells.
pub fn cells_in_selection(doc: &Node, sel: Selection) -> Vec<usize> {
    let Selection::Cell { anchor, head } = sel else {
        return Vec::new();
    };
    let Ok(rp) = ResolvedPos::resolve(doc, anchor.min(head) + 1) else {
        return Vec::new();
    };
    let Some((td, _, _)) = find_table(&rp) else {
        return Vec::new();
    };
    let table = rp.node(td);
    let tstart = rp.before(td);
    let map = TableMap::of(table);
    let (Some((ar, ac)), Some((hr, hc))) = (
        logical_at_pos(table, tstart, &map, anchor),
        logical_at_pos(table, tstart, &map, head),
    ) else {
        return Vec::new();
    };
    let (mut r0, mut r1) = (ar.min(hr), ar.max(hr));
    let (mut c0, mut c1) = (ac.min(hc), ac.max(hc));
    // Expand to whole spans so the highlight matches the eventual merge.
    let placements = placements_of(table);
    loop {
        let before = (r0, r1, c0, c1);
        for p in &placements {
            let pr1 = p.row + p.rowspan - 1;
            let pc1 = p.col + p.colspan - 1;
            if p.row <= r1 && pr1 >= r0 && p.col <= c1 && pc1 >= c0 {
                r0 = r0.min(p.row);
                r1 = r1.max(pr1);
                c0 = c0.min(p.col);
                c1 = c1.max(pc1);
            }
        }
        if (r0, r1, c0, c1) == before {
            break;
        }
    }
    let mut out: Vec<usize> = Vec::new();
    for r in r0..=r1 {
        for c in c0..=c1 {
            if let Some((ri, ci)) = map.at(r, c) {
                let pos = cell_before_abs(table, tstart, ri, ci);
                if !out.contains(&pos) {
                    out.push(pos);
                }
            }
        }
    }
    out
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

/// Resolve an absolute cell-before position into its logical `(row, col)`.
fn logical_at_pos(
    table: &Node,
    tstart: usize,
    map: &TableMap,
    pos: usize,
) -> Option<(usize, usize)> {
    for (ri, row) in table.content().iter().enumerate() {
        for (ci, _) in row.content().iter().enumerate() {
            if cell_before_abs(table, tstart, ri, ci) == pos {
                return map.logical_of((ri, ci));
            }
        }
    }
    None
}

/// Concatenated, de-duplicated block content for a merge: drop empty
/// paragraphs, but keep at least one block so the cell stays valid.
fn merged_content(schema: &Schema, blocks: Vec<Node>) -> Vec<Node> {
    let non_empty: Vec<Node> = blocks
        .into_iter()
        .filter(|b| b.content().size() > 0)
        .collect();
    if non_empty.is_empty() {
        schema
            .node("paragraph", Default::default(), vec![], vec![])
            .ok()
            .into_iter()
            .collect()
    } else {
        non_empty
    }
}

/// Merge the cells covered by the current [`Selection::Cell`] into one,
/// with the matching colspan/rowspan and the concatenated content of all
/// merged cells. The selection rectangle is expanded to whole spans so the
/// merge always operates on a clean rectangle; the render then compacts any
/// rows/columns that become fully covered, so no orphan spans remain. A
/// no-op unless the rectangle covers more than one cell.
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

        let (Some((ar, ac)), Some((hr, hc))) = (
            logical_at_pos(table, tstart, &map, anchor),
            logical_at_pos(table, tstart, &map, head),
        ) else {
            return false;
        };
        let (mut r0, mut r1) = (ar.min(hr), ar.max(hr));
        let (mut c0, mut c1) = (ac.min(hc), ac.max(hc));

        let placements = placements_of(table);
        // Expand the rectangle until it covers whole spans (no cell pokes out).
        loop {
            let before = (r0, r1, c0, c1);
            for p in &placements {
                let pr1 = p.row + p.rowspan - 1;
                let pc1 = p.col + p.colspan - 1;
                let overlaps = p.row <= r1 && pr1 >= r0 && p.col <= c1 && pc1 >= c0;
                if overlaps {
                    r0 = r0.min(p.row);
                    r1 = r1.max(pr1);
                    c0 = c0.min(p.col);
                    c1 = c1.max(pc1);
                }
            }
            if (r0, r1, c0, c1) == before {
                break;
            }
        }
        if r0 == r1 && c0 == c1 {
            return false; // a single (possibly spanned) cell — nothing to merge
        }

        let schema = state.schema();
        let mut sorted = placements.clone();
        sorted.sort_by_key(|p| (p.row, p.col));
        let mut merged_blocks: Vec<Node> = Vec::new();
        let mut top_left_attrs = Attrs::new();
        let mut others: Vec<Placement> = Vec::new();
        for p in sorted {
            let inside = p.row >= r0
                && p.row + p.rowspan - 1 <= r1
                && p.col >= c0
                && p.col + p.colspan - 1 <= c1;
            if inside {
                merged_blocks.extend(p.cell.content().children().to_vec());
                if (p.row, p.col) == (r0, c0) {
                    top_left_attrs = p.cell.attrs().clone();
                }
            } else {
                others.push(p);
            }
        }
        top_left_attrs.insert("colspan".into(), AttrValue::from((c1 - c0 + 1) as u64));
        top_left_attrs.insert("rowspan".into(), AttrValue::from((r1 - r0 + 1) as u64));
        let blocks = merged_content(schema, merged_blocks);
        let Ok(merged_cell) = schema.node("table_cell", top_left_attrs, blocks, vec![]) else {
            return false;
        };
        others.push(Placement {
            row: r0,
            col: c0,
            colspan: c1 - c0 + 1,
            rowspan: r1 - r0 + 1,
            cell: merged_cell,
        });
        let Some(new_table) = render_placements(schema, &others, table.attrs().clone()) else {
            return false;
        };
        replace_table_logical(state, &rp, td, new_table, (r0, c0), dispatch);
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
        let Some((td, lrow, lcol)) = caret_logical(&rp) else {
            return false;
        };
        let table = rp.node(td);
        let schema = state.schema();
        let mut placements = placements_of(table);
        let Some(idx) = placements
            .iter()
            .position(|p| p.row == lrow && p.col == lcol)
        else {
            return false;
        };
        let p = placements[idx].clone();
        if p.colspan == 1 && p.rowspan == 1 {
            return false;
        }
        // Keep the origin as a 1×1 cell with the content; fill the rest of
        // the old span with fresh empty 1×1 cells.
        let mut kept_attrs = p.cell.attrs().clone();
        kept_attrs.insert("colspan".into(), AttrValue::from(1u64));
        kept_attrs.insert("rowspan".into(), AttrValue::from(1u64));
        let Ok(kept) = schema.node(
            "table_cell",
            kept_attrs,
            p.cell.content().children().to_vec(),
            p.cell.marks().to_vec(),
        ) else {
            return false;
        };
        placements[idx] = Placement {
            row: p.row,
            col: p.col,
            colspan: 1,
            rowspan: 1,
            cell: kept,
        };
        for dr in 0..p.rowspan {
            for dc in 0..p.colspan {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let Some(cell) = empty_cell(schema) else {
                    return false;
                };
                placements.push(Placement {
                    row: p.row + dr,
                    col: p.col + dc,
                    colspan: 1,
                    rowspan: 1,
                    cell,
                });
            }
        }
        let Some(new_table) = render_placements(schema, &placements, table.attrs().clone()) else {
            return false;
        };
        replace_table_logical(state, &rp, td, new_table, (lrow, lcol), dispatch);
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
