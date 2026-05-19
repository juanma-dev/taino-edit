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

    /// Build a fragment from a sequence of sibling nodes.
    pub fn from_nodes(content: Vec<Node>) -> Fragment {
        let size = content.iter().map(|n| n.node_size()).sum();
        Fragment(Arc::new(FragmentInner { content, size }))
    }

    /// Build a single-node fragment.
    pub fn from_node(node: Node) -> Fragment {
        Fragment::from_nodes(vec![node])
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

    /// The first child, if any.
    pub fn first_child(&self) -> Option<&Node> {
        self.0.content.first()
    }

    /// The last child, if any.
    pub fn last_child(&self) -> Option<&Node> {
        self.0.content.last()
    }

    /// Slice this fragment between content positions `from..to`. Partially
    /// covered children are themselves cut.
    pub fn cut(&self, from: usize, to: usize) -> Fragment {
        if from == 0 && to == self.0.size {
            return self.clone();
        }
        let mut result = Vec::new();
        if to > from {
            let mut pos = 0;
            for child in &self.0.content {
                if pos >= to {
                    break;
                }
                let end = pos + child.node_size();
                if end > from {
                    let piece = if pos < from || end > to {
                        if child.is_text() {
                            child.cut(from.saturating_sub(pos), (to - pos).min(child.node_size()))
                        } else {
                            child.cut(
                                from.saturating_sub(pos + 1),
                                (to - pos - 1).min(child.content().size()),
                            )
                        }
                    } else {
                        child.clone()
                    };
                    result.push(piece);
                }
                pos = end;
            }
        }
        Fragment::from_nodes(result)
    }

    /// Concatenate two fragments, merging a trailing/leading text run pair
    /// with identical markup so text never fragments spuriously.
    pub fn append(&self, other: &Fragment) -> Fragment {
        if other.0.size == 0 {
            return self.clone();
        }
        if self.0.size == 0 {
            return other.clone();
        }
        let mut content = self.0.content.clone();
        let mut start = 0;
        let last = content.len() - 1;
        if let (Some(l), Some(f)) = (content.last(), other.0.content.first()) {
            if l.is_text() && f.is_text() && l.same_markup(f) {
                let merged = l.with_text(format!(
                    "{}{}",
                    l.text().unwrap_or(""),
                    f.text().unwrap_or("")
                ));
                content[last] = merged;
                start = 1;
            }
        }
        content.extend(other.0.content[start..].iter().cloned());
        Fragment::from_nodes(content)
    }

    /// Return a copy with the child at `index` replaced by `node`.
    pub(crate) fn replace_child(&self, index: usize, node: Node) -> Fragment {
        if self.0.content[index] == node {
            return self.clone();
        }
        let mut content = self.0.content.clone();
        content[index] = node;
        Fragment::from_nodes(content)
    }

    pub(crate) fn from_vec(content: Vec<Node>) -> Fragment {
        Fragment::from_nodes(content)
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
