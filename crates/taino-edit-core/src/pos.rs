//! [`ResolvedPos`] — an absolute document position resolved into the path of
//! ancestor nodes that contain it.

use crate::error::DocError;
use crate::node::Node;

#[derive(Debug, Clone)]
struct PathStep {
    node: Node,
    index: usize,
    /// Absolute position just inside `node` (before its first child).
    start: usize,
}

/// An absolute position together with its enclosing-node path.
///
/// Produced by [`resolve`](ResolvedPos::resolve). Depth 0 is the root passed
/// to `resolve` (normally the document node).
#[derive(Debug, Clone)]
pub struct ResolvedPos {
    pos: usize,
    path: Vec<PathStep>,
    parent_offset: usize,
}

impl ResolvedPos {
    /// Resolve `pos` against `root` (treated as the document).
    ///
    /// Returns [`DocError::PositionOutOfRange`] if `pos` is not in
    /// `0..=root.content().size()`.
    pub fn resolve(root: &Node, pos: usize) -> Result<ResolvedPos, DocError> {
        let max = root.content().size();
        if pos > max {
            return Err(DocError::PositionOutOfRange { pos, max });
        }
        let mut path = Vec::new();
        let mut node = root.clone();
        let mut start = 0usize;
        let mut parent_offset = pos;
        loop {
            let (index, offset) = node.content().find_index(parent_offset);
            let rem = parent_offset - offset;
            path.push(PathStep {
                node: node.clone(),
                index,
                start: start + offset,
            });
            if rem == 0 {
                break;
            }
            let child = node.child(index).clone();
            if child.is_text() {
                break;
            }
            start += offset + 1;
            parent_offset = rem - 1;
            node = child;
        }
        Ok(ResolvedPos {
            pos,
            path,
            parent_offset,
        })
    }

    /// The absolute position.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// The deepest depth (0 = root).
    pub fn depth(&self) -> usize {
        self.path.len() - 1
    }

    /// The offset of this position within its immediate parent's content.
    pub fn parent_offset(&self) -> usize {
        self.parent_offset
    }

    /// The ancestor node at `depth`.
    ///
    /// # Panics
    /// Panics if `depth > self.depth()`.
    pub fn node(&self, depth: usize) -> &Node {
        &self.path[depth].node
    }

    /// The immediate parent of this position.
    pub fn parent(&self) -> &Node {
        &self.path[self.depth()].node
    }

    /// The document (root) node.
    pub fn doc(&self) -> &Node {
        &self.path[0].node
    }

    /// The child index into `node(depth)` that this position points at or
    /// into.
    pub fn index(&self, depth: usize) -> usize {
        self.path[depth].index
    }

    /// Absolute position just inside `node(depth)` (before its first child).
    /// `start(0)` is always 0.
    pub fn start(&self, depth: usize) -> usize {
        if depth == 0 {
            0
        } else {
            self.path[depth - 1].start + 1
        }
    }

    /// Absolute position just after the last child of `node(depth)`.
    pub fn end(&self, depth: usize) -> usize {
        self.start(depth) + self.path[depth].node.content().size()
    }

    /// Position directly before `node(depth)` (for `1 <= depth <= self.depth()`).
    ///
    /// # Panics
    /// Panics if `depth == 0` (the root has no position before it).
    pub fn before(&self, depth: usize) -> usize {
        assert!(depth >= 1, "no position before the root");
        self.path[depth - 1].start
    }

    /// Position directly after `node(depth)` (for `1 <= depth <= self.depth()`).
    ///
    /// # Panics
    /// Panics if `depth == 0` (the root has no position after it).
    pub fn after(&self, depth: usize) -> usize {
        assert!(depth >= 1, "no position after the root");
        self.path[depth - 1].start + self.path[depth].node.node_size()
    }

    /// The offset into the text node at this position. Zero when the position
    /// is not inside a text run.
    pub fn text_offset(&self) -> usize {
        self.pos - self.path[self.depth()].start
    }
}
