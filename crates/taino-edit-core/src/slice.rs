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

    /// Insert `fragment` into this slice at content position `pos`
    /// (accounting for the open start). `None` if it cannot be placed flatly.
    pub fn insert_at(&self, pos: usize, fragment: Fragment) -> Option<Slice> {
        let content = insert_into(&self.content, pos + self.open_start, fragment)?;
        Some(Slice::new(content, self.open_start, self.open_end))
    }

    /// Remove the flat range `from..to` from this slice's content. `None` if
    /// the range is not flat (crosses a non-text node boundary).
    pub fn remove_between(&self, from: usize, to: usize) -> Option<Slice> {
        let content = remove_range(&self.content, from + self.open_start, to + self.open_start)?;
        Some(Slice::new(content, self.open_start, self.open_end))
    }
}

fn insert_into(content: &Fragment, dist: usize, insert: Fragment) -> Option<Fragment> {
    let (index, offset) = content.find_index(dist);
    let child = content.children().get(index);
    if offset == dist || child.is_some_and(|c| c.is_text()) {
        return Some(
            content
                .cut(0, dist)
                .append(&insert)
                .append(&content.cut(dist, content.size())),
        );
    }
    let child = child?;
    let inner = insert_into(child.content(), dist - offset - 1, insert)?;
    Some(content.replace_child(index, child.copy_content(inner)))
}

fn remove_range(content: &Fragment, from: usize, to: usize) -> Option<Fragment> {
    let (index, offset) = content.find_index(from);
    let child = content.children().get(index);
    let (index_to, offset_to) = content.find_index(to);
    if offset == from || child.is_some_and(|c| c.is_text()) {
        if offset_to != to
            && !content
                .children()
                .get(index_to)
                .is_some_and(|c| c.is_text())
        {
            return None;
        }
        return Some(
            content
                .cut(0, from)
                .append(&content.cut(to, content.size())),
        );
    }
    if index != index_to {
        return None;
    }
    let child = child?;
    let inner = remove_range(child.content(), from - offset - 1, to - offset - 1)?;
    Some(content.replace_child(index, child.copy_content(inner)))
}
