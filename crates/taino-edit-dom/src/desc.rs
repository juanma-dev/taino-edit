//! [`ViewDesc`] — the bridge data structure that mirrors the document tree
//! to DOM nodes so a state change can be diffed and patched incrementally.
//!
//! A `ViewDesc` is built when the view first mounts and is updated in place
//! on subsequent state updates. Each node in the document has a corresponding
//! `ViewDesc` whose `dom` is the element/text node rendered for it; an
//! `Element` desc holds `children` desc nodes mirroring its document
//! children, while a `Text` desc carries the marked text node and (for
//! mark-wrapped text) the outermost wrapping element as the `dom`.

use taino_edit_core::Node;
use web_sys::{Element, Text};

/// A document-node ↔ DOM-node correspondence.
#[derive(Debug, Clone)]
pub enum ViewDesc {
    /// An element node (block or inline non-text).
    Element {
        /// The document node this DOM element represents.
        node: Node,
        /// The DOM element rendered for `node`. Children of `node` live
        /// inside this element.
        dom: Element,
        /// Descriptors for `node.content()` children, in order.
        children: Vec<ViewDesc>,
    },
    /// A text run; `dom` is the outermost wrapper if marks are applied, or
    /// the raw text node when there are no marks.
    Text {
        /// The document text node.
        node: Node,
        /// The raw DOM text node carrying the characters.
        text: Text,
        /// The outermost mark-wrapper element, when marks wrap the text; the
        /// raw `text` node is the DOM child when this is `None`.
        wrapper: Option<Element>,
    },
}

impl ViewDesc {
    /// The DOM node to insert into the parent (the wrapper for marked text,
    /// the bare text node otherwise, or the element for elements).
    pub fn dom_node(&self) -> web_sys::Node {
        match self {
            ViewDesc::Element { dom, .. } => dom.clone().into(),
            ViewDesc::Text {
                text,
                wrapper: None,
                ..
            } => text.clone().into(),
            ViewDesc::Text {
                wrapper: Some(w), ..
            } => w.clone().into(),
        }
    }

    /// The document node this descriptor describes.
    pub fn node(&self) -> &Node {
        match self {
            ViewDesc::Element { node, .. } | ViewDesc::Text { node, .. } => node,
        }
    }
}
