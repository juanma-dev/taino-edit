//! Editor [`Selection`]. v0.1 covers the three ProseMirror selection shapes;
//! mapping is positional (sufficient for the linear, single-user editing
//! v0.1 targets — richer "find a valid selection nearby" behaviour is a
//! v0.2 refinement).

use crate::map::Mapping;
use crate::node::Node;

/// What is currently selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// A (possibly empty) text range. `anchor` is the fixed side, `head` the
    /// moving side; they may be in either order.
    Text {
        /// Fixed end of the selection.
        anchor: usize,
        /// Moving end (where the caret is).
        head: usize,
    },
    /// A single node selected as a unit, starting at `pos`.
    Node {
        /// Position directly before the selected node.
        pos: usize,
    },
    /// The whole document.
    All,
}

impl Selection {
    /// A collapsed text caret at `pos`.
    pub fn caret(pos: usize) -> Selection {
        Selection::Text {
            anchor: pos,
            head: pos,
        }
    }

    /// Lowest selected position.
    pub fn from(&self) -> usize {
        match self {
            Selection::Text { anchor, head } => (*anchor).min(*head),
            Selection::Node { pos } => *pos,
            Selection::All => 0,
        }
    }

    /// Highest selected position (relative to `doc` for `All`/`Node`).
    pub fn to(&self, doc: &Node) -> usize {
        match self {
            Selection::Text { anchor, head } => (*anchor).max(*head),
            Selection::Node { pos } => doc.node_at(*pos).map_or(*pos, |n| pos + n.node_size()),
            Selection::All => doc.content().size(),
        }
    }

    /// Whether the selection is an empty caret.
    pub fn is_empty(&self) -> bool {
        matches!(self, Selection::Text { anchor, head } if anchor == head)
    }

    /// Map this selection forward through `mapping`, clamping into `doc`.
    /// A node selection whose node was touched degrades to a caret.
    pub fn map(&self, doc: &Node, mapping: &Mapping) -> Selection {
        let max = doc.content().size();
        let clamp = |p: usize| p.min(max);
        match self {
            Selection::Text { anchor, head } => Selection::Text {
                anchor: clamp(mapping.map(*anchor, 1)),
                head: clamp(mapping.map(*head, 1)),
            },
            Selection::Node { pos } => {
                let r = mapping.map_result(*pos, 1);
                if r.deleted_after() {
                    Selection::caret(clamp(r.pos))
                } else {
                    Selection::Node { pos: clamp(r.pos) }
                }
            }
            Selection::All => Selection::All,
        }
    }
}
