//! [`Slice`] — a [`Fragment`] plus the open depths at its two ends, the unit
//! produced by cut and consumed by paste/replace operations.

use crate::fragment::Fragment;

/// A piece of a document. `open_start`/`open_end` record how many levels of
/// node are left "open" (not closed off) at each side, so the slice can be
/// stitched back into a tree at the cut boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    content: Fragment,
    open_start: usize,
    open_end: usize,
}

impl Slice {
    /// The empty slice (no content, both ends closed).
    pub fn empty() -> Slice {
        Slice {
            content: Fragment::empty(),
            open_start: 0,
            open_end: 0,
        }
    }

    /// Construct a slice from a fragment and its open depths.
    pub fn new(content: Fragment, open_start: usize, open_end: usize) -> Slice {
        Slice {
            content,
            open_start,
            open_end,
        }
    }

    /// The slice's content fragment.
    pub fn content(&self) -> &Fragment {
        &self.content
    }

    /// Open depth at the start.
    pub fn open_start(&self) -> usize {
        self.open_start
    }

    /// Open depth at the end.
    pub fn open_end(&self) -> usize {
        self.open_end
    }

    /// Size of the slice in document positions, accounting for the open
    /// depths at each side.
    pub fn size(&self) -> usize {
        self.content.size() - self.open_start - self.open_end
    }

    /// Whether the slice has no content.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}
