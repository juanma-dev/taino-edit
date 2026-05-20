//! [`Decoration`]s — visual overlays applied on top of the rendered DOM
//! without changing the document. The DOM bridge applies/removes them
//! against the matching descriptors.
//!
//! v0.1 ships **node decorations** only (a CSS class on a block element),
//! which is enough to drive most adapter-side UI (selection highlights,
//! collaboration cursors, slash-menu targets). Range-level inline
//! decorations are tracked as a v0.2 item — they need text-node splitting
//! that interacts non-trivially with the diff/patch.

/// A decoration applied on top of the rendered DOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoration {
    /// Apply a CSS class to the block element whose start position is `pos`.
    Node {
        /// Document position directly before the decorated node.
        pos: usize,
        /// CSS class to add to the block's DOM element.
        class: String,
    },
}
