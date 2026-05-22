//! `taino-edit-table-view` — pointer interaction for taino-edit tables.
//!
//! Provides [`TableView`], a [`taino_edit_dom::ViewPlugin`] that adds the
//! mouse behaviour a table editor needs on top of the (framework-agnostic)
//! `Table` extension's schema + commands:
//!
//! * **cell drag-select** — press in a cell and drag across others to build
//!   a [`Selection::Cell`] (which `merge_cells` then operates on);
//! * **selection highlight** — selected cells get the
//!   `taino-cell-selected` CSS class via decorations;
//! * **column resize** — press near a cell's right border and drag to set
//!   the column width (reuses `set_column_width`).
//!
//! Adapters install it with
//! [`EditorView::set_view_plugins`](taino_edit_dom::EditorView::set_view_plugins)
//! and wire `mousedown`/`mousemove`/`mouseup` to
//! [`EditorView::handle_view_event`](taino_edit_dom::EditorView::handle_view_event),
//! refreshing decorations through
//! [`EditorView::refresh_view_decorations`](taino_edit_dom::EditorView::refresh_view_decorations).

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(missing_docs, rust_2018_idioms)]

use std::cell::RefCell;

use taino_edit_core::Selection;
use taino_edit_dom::{Decoration, EditorView, ViewAction, ViewPlugin};
use taino_edit_extensions::{cell_at, cells_in_selection, set_column_width};
use wasm_bindgen::JsCast;

/// CSS class applied to the DOM element of each selected cell.
pub const SELECTED_CELL_CLASS: &str = "taino-cell-selected";

/// How close (in px) to a cell's right border a press must be to start a
/// column resize instead of a cell drag-select.
const RESIZE_GRIP_PX: f64 = 6.0;

struct DragState {
    /// Position before the cell where the press started.
    anchor_cell_pos: usize,
}

struct ResizeState {
    /// Logical column being resized.
    col: usize,
    /// Pointer x at press time.
    start_x: f64,
    /// Column width at press time.
    start_width: f64,
}

/// The table pointer-interaction plugin. Stateless across editors — install
/// one per `EditorView`.
#[derive(Default)]
pub struct TableView {
    drag: RefCell<Option<DragState>>,
    resize: RefCell<Option<ResizeState>>,
}

impl TableView {
    /// A fresh plugin.
    pub fn new() -> Self {
        Self::default()
    }
}

fn mouse(ev: &web_sys::Event) -> Option<web_sys::MouseEvent> {
    ev.clone().dyn_into::<web_sys::MouseEvent>().ok()
}

impl ViewPlugin for TableView {
    fn handle_event(&self, view: &EditorView, ev: &web_sys::Event) -> Option<ViewAction> {
        let me = mouse(ev)?;
        let (x, y) = (me.client_x() as f64, me.client_y() as f64);
        match ev.type_().as_str() {
            "mousedown" => {
                self.drag.replace(None);
                self.resize.replace(None);
                let pos = view.pos_at_point(x as f32, y as f32)?;
                let cell = cell_at(view.doc(), pos)?;
                // Resize if the press is near the cell's right border.
                if let Some(rect) = view
                    .node_dom_at(cell.cell_pos)
                    .map(|e| e.get_bounding_client_rect())
                {
                    if (rect.right() - x).abs() <= RESIZE_GRIP_PX {
                        self.resize.replace(Some(ResizeState {
                            col: cell.col,
                            start_x: x,
                            start_width: rect.width(),
                        }));
                        ev.prevent_default();
                        return None;
                    }
                }
                self.drag.replace(Some(DragState {
                    anchor_cell_pos: cell.cell_pos,
                }));
                None
            }
            "mousemove" => {
                // A resize commits only on mouseup; nothing to do mid-move.
                if self.resize.borrow().is_some() {
                    ev.prevent_default();
                    return None;
                }
                let anchor = self.drag.borrow().as_ref().map(|d| d.anchor_cell_pos)?;
                let pos = view.pos_at_point(x as f32, y as f32)?;
                let cell = cell_at(view.doc(), pos)?;
                if cell.cell_pos != anchor {
                    // Dragging across cells → a rectangular cell selection.
                    ev.prevent_default();
                    return Some(ViewAction::Select(Selection::Cell {
                        anchor,
                        head: cell.cell_pos,
                    }));
                }
                None
            }
            "mouseup" => {
                let resize = self.resize.replace(None);
                self.drag.replace(None);
                if let Some(r) = resize {
                    let new_w = (r.start_width + (x - r.start_x)).round().max(1.0) as u64;
                    return Some(ViewAction::Command(set_column_width(r.col, new_w)));
                }
                None
            }
            _ => None,
        }
    }

    fn decorations(&self, view: &EditorView, selection: Option<Selection>) -> Vec<Decoration> {
        let Some(sel) = selection else {
            return Vec::new();
        };
        cells_in_selection(view.doc(), sel)
            .into_iter()
            .map(|pos| Decoration::Node {
                pos,
                class: SELECTED_CELL_CLASS.to_string(),
            })
            .collect()
    }
}
