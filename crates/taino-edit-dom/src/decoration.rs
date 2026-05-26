//! [`Decoration`]s — visual overlays applied on top of the rendered DOM
//! without changing the document. The DOM bridge applies/removes them
//! against the matching descriptors.
//!
//! Two kinds ship today:
//!
//! * **Node decorations** add a CSS class to the block (or nested) element
//!   whose start position is `pos` — used for selection highlights,
//!   collaboration cursors on a block, slash-menu targets, table cell
//!   highlighting, etc.
//! * **Inline (range-level) decorations** highlight an arbitrary inline range
//!   `[from, to)`. They are drawn as an **overlay** — absolutely-positioned
//!   boxes layered above the text — rather than by wrapping the text in a
//!   `<span>`. Wrapping would split the editable text nodes, which the
//!   diff/patch read-back ([`EditorView::read_dom_changes`]) reads by
//!   `text.data()`; the overlay leaves the editable DOM untouched, so typing
//!   and reconciliation are unaffected. This is exactly what third-party UI
//!   needs: search highlights, comment ranges, collaborative remote
//!   selections.
//!
//! [`EditorView::read_dom_changes`]: crate::EditorView::read_dom_changes

/// A decoration applied on top of the rendered DOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoration {
    /// Apply a CSS class to the element whose start position is `pos` (a
    /// top-level block or a nested node such as a table cell).
    Node {
        /// Document position directly before the decorated node.
        pos: usize,
        /// CSS class to add to the node's DOM element.
        class: String,
    },
    /// Highlight the inline range `[from, to)` with a CSS class, drawn as an
    /// overlay above the text. It does **not** alter the editable DOM, so
    /// typing and the diff/patch are unaffected. The class is applied to each
    /// overlay box (one per client rect, so multi-line ranges render as
    /// several boxes).
    Inline {
        /// Document position at the start of the range (inclusive).
        from: usize,
        /// Document position at the end of the range (exclusive).
        to: usize,
        /// CSS class added to each overlay box covering the range.
        class: String,
    },
}

impl Decoration {
    /// A [node decoration](Decoration::Node): add `class` to the element at
    /// `pos`.
    pub fn node(pos: usize, class: impl Into<String>) -> Self {
        Decoration::Node {
            pos,
            class: class.into(),
        }
    }

    /// An [inline decoration](Decoration::Inline): highlight `[from, to)` with
    /// `class`, drawn as an overlay.
    pub fn inline(from: usize, to: usize, class: impl Into<String>) -> Self {
        Decoration::Inline {
            from: from.min(to),
            to: from.max(to),
            class: class.into(),
        }
    }
}
