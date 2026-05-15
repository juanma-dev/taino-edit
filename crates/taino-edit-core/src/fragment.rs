//! [`Fragment`] — an immutable, ordered sequence of sibling [`Node`]s.

use std::sync::Arc;

use crate::node::Node;

#[derive(Debug)]
pub(crate) struct FragmentInner {
    content: Vec<Node>,
    size: usize,
}

/// An immutable, ordered run of sibling nodes. Cloning is O(1) (an [`Arc`]
/// bump); structural sharing is intentional.
#[derive(Debug, Clone)]
pub struct Fragment(Arc<FragmentInner>);

impl Fragment {
    /// The empty fragment.
    pub fn empty() -> Fragment {
        Fragment(Arc::new(FragmentInner {
            content: Vec::new(),
            size: 0,
        }))
    }

    pub(crate) fn from_nodes(content: Vec<Node>) -> Fragment {
        let size = content.iter().map(|n| n.node_size()).sum();
        Fragment(Arc::new(FragmentInner { content, size }))
    }

    /// Number of direct children.
    pub fn child_count(&self) -> usize {
        self.0.content.len()
    }

    /// Whether the fragment has no children.
    pub fn is_empty(&self) -> bool {
        self.0.content.is_empty()
    }

    /// Borrow the child at index `i`.
    ///
    /// # Panics
    /// Panics if `i >= child_count()`.
    pub fn child(&self, i: usize) -> &Node {
        &self.0.content[i]
    }

    /// All children as a slice.
    pub fn children(&self) -> &[Node] {
        &self.0.content
    }

    /// Iterate the children.
    pub fn iter(&self) -> std::slice::Iter<'_, Node> {
        self.0.content.iter()
    }

    /// Summed [`Node::node_size`] of all children — i.e. the number of
    /// addressable positions inside the parent this fragment fills.
    pub fn size(&self) -> usize {
        self.0.size
    }

    /// Find the child index containing or starting at content position `pos`,
    /// returning `(index, offset_at_index_start)`.
    ///
    /// `pos` must be in `0..=size`. A `pos` exactly on a child boundary
    /// resolves to the child that *starts* there (or `child_count` at the
    /// end).
    pub(crate) fn find_index(&self, pos: usize) -> (usize, usize) {
        if pos == 0 {
            return (0, 0);
        }
        let mut offset = 0;
        for (i, child) in self.0.content.iter().enumerate() {
            let end = offset + child.node_size();
            if pos == offset {
                return (i, offset);
            }
            if pos < end {
                return (i, offset);
            }
            if pos == end {
                return (i + 1, end);
            }
            offset = end;
        }
        (self.0.content.len(), offset)
    }
}

impl PartialEq for Fragment {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0.content == other.0.content
    }
}
impl Eq for Fragment {}

impl<'a> IntoIterator for &'a Fragment {
    type Item = &'a Node;
    type IntoIter = std::slice::Iter<'a, Node>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.content.iter()
    }
}
