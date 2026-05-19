//! The tree-replace algorithm: substitute the document range `from..to`
//! with a [`Slice`], producing a new, schema-valid document.
//!
//! This is a faithful port of ProseMirror's `replace.ts` (the most subtle
//! piece of the model). It is purely functional — nodes are immutable, so
//! `replace` returns a fresh root and the old tree is untouched, which is
//! exactly what invertible [`Step`](crate::Step)s and undo/redo need.

use std::fmt;

use crate::error::DocError;
use crate::fragment::Fragment;
use crate::node::Node;
use crate::pos::ResolvedPos;
use crate::schema::Schema;
use crate::slice::Slice;

/// Why a [`Node::replace`] could not be performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceError {
    /// The slice's open depth exceeds the insertion position's depth.
    OpenTooDeep,
    /// `from`'s and `to`'s open depths are inconsistent with the slice.
    InconsistentOpenDepths,
    /// Two nodes that would have to be joined have incompatible content.
    CannotJoin {
        /// The node type being joined on.
        onto: String,
        /// The node type being joined.
        joined: String,
    },
    /// The resulting content would violate the schema for `parent`.
    InvalidContent {
        /// The parent node type whose content became invalid.
        parent: String,
    },
    /// A boundary position was out of range.
    Position {
        /// The offending position.
        pos: usize,
        /// The maximum valid position.
        max: usize,
    },
}

impl fmt::Display for ReplaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplaceError::OpenTooDeep => {
                write!(f, "inserted content deeper than the insertion position")
            }
            ReplaceError::InconsistentOpenDepths => write!(f, "inconsistent open depths"),
            ReplaceError::CannotJoin { onto, joined } => {
                write!(f, "cannot join `{joined}` onto `{onto}`")
            }
            ReplaceError::InvalidContent { parent } => {
                write!(f, "replacement violates the schema for `{parent}`")
            }
            ReplaceError::Position { pos, max } => {
                write!(f, "position {pos} out of range (max {max})")
            }
        }
    }
}

impl std::error::Error for ReplaceError {}

fn map_pos(e: DocError) -> ReplaceError {
    match e {
        DocError::PositionOutOfRange { pos, max } => ReplaceError::Position { pos, max },
        _ => ReplaceError::Position { pos: 0, max: 0 },
    }
}

impl Node {
    /// Replace the range `from..to` with `slice`, returning the new root.
    ///
    /// The document is treated as the root. Content that would violate the
    /// schema (including illegal joins at the boundaries) is rejected with a
    /// [`ReplaceError`] rather than silently produced.
    pub fn replace(
        &self,
        from: usize,
        to: usize,
        slice: &Slice,
        schema: &Schema,
    ) -> Result<Node, ReplaceError> {
        let rf = ResolvedPos::resolve(self, from).map_err(map_pos)?;
        let rt = ResolvedPos::resolve(self, to).map_err(map_pos)?;
        replace(&rf, &rt, slice, schema)
    }

    /// Extract the content between `from` and `to` as a [`Slice`], recording
    /// how deeply each end is open so it can be re-inserted faithfully.
    pub fn slice(&self, from: usize, to: usize) -> Result<Slice, DocError> {
        if from == to {
            return Ok(Slice::empty());
        }
        let rf = ResolvedPos::resolve(self, from)?;
        let rt = ResolvedPos::resolve(self, to)?;
        let depth = rf.shared_depth(to);
        let start = rf.start(depth);
        let node = rf.node(depth);
        let content = node.content().cut(rf.pos() - start, rt.pos() - start);
        Ok(Slice::new(content, rf.depth() - depth, rt.depth() - depth))
    }
}

fn replace(
    from: &ResolvedPos,
    to: &ResolvedPos,
    slice: &Slice,
    schema: &Schema,
) -> Result<Node, ReplaceError> {
    if slice.open_start() > from.depth() {
        return Err(ReplaceError::OpenTooDeep);
    }
    if from.depth() as isize - slice.open_start() as isize
        != to.depth() as isize - slice.open_end() as isize
    {
        return Err(ReplaceError::InconsistentOpenDepths);
    }
    replace_outer(from, to, slice, 0, schema)
}

fn close(schema: &Schema, template: &Node, content: Fragment) -> Result<Node, ReplaceError> {
    if !schema.fragment_valid(template.node_type(), &content) {
        return Err(ReplaceError::InvalidContent {
            parent: template.node_type().name().to_string(),
        });
    }
    Ok(template.copy_content(content))
}

/// Validate that a node of type `joined` can be joined onto `onto`.
fn check_join(schema: &Schema, onto: &Node, joined: &Node) -> Result<(), ReplaceError> {
    if !schema.types_compatible(joined.node_type(), onto.node_type()) {
        return Err(ReplaceError::CannotJoin {
            onto: onto.node_type().name().to_string(),
            joined: joined.node_type().name().to_string(),
        });
    }
    Ok(())
}

fn joinable(
    schema: &Schema,
    before: &ResolvedPos,
    after: &ResolvedPos,
    depth: usize,
) -> Result<Node, ReplaceError> {
    let node = before.node(depth).clone();
    check_join(schema, &node, after.node(depth))?;
    Ok(node)
}

fn add_node(child: Node, target: &mut Vec<Node>) {
    if let Some(last) = target.last() {
        if child.is_text() && last.is_text() && child.same_markup(last) {
            let merged = child.with_text(format!(
                "{}{}",
                last.text().unwrap_or(""),
                child.text().unwrap_or("")
            ));
            let n = target.len() - 1;
            target[n] = merged;
            return;
        }
    }
    target.push(child);
}

fn add_range(
    start: Option<&ResolvedPos>,
    end: Option<&ResolvedPos>,
    depth: usize,
    target: &mut Vec<Node>,
) {
    let anchor = end.or(start).expect("add_range needs a bound");
    let node = anchor.node(depth);
    let mut start_index = 0;
    let end_index = match end {
        Some(e) => e.index(depth),
        None => node.child_count(),
    };
    if let Some(s) = start {
        start_index = s.index(depth);
        if s.depth() > depth {
            start_index += 1;
        } else if s.text_offset() > 0 {
            add_node(s.node_after().expect("node after start"), target);
            start_index += 1;
        }
    }
    for i in start_index..end_index {
        add_node(node.child(i).clone(), target);
    }
    if let Some(e) = end {
        if e.depth() == depth && e.text_offset() > 0 {
            add_node(e.node_before().expect("node before end"), target);
        }
    }
}

fn replace_two_way(
    from: &ResolvedPos,
    to: &ResolvedPos,
    depth: usize,
    schema: &Schema,
) -> Result<Fragment, ReplaceError> {
    let mut content = Vec::new();
    add_range(None, Some(from), depth, &mut content);
    if from.depth() > depth {
        let template = joinable(schema, from, to, depth + 1)?;
        let inner = replace_two_way(from, to, depth + 1, schema)?;
        add_node(close(schema, &template, inner)?, &mut content);
    }
    add_range(Some(to), None, depth, &mut content);
    Ok(Fragment::from_vec(content))
}

#[allow(clippy::too_many_arguments)]
fn replace_three_way(
    from: &ResolvedPos,
    start: &ResolvedPos,
    end: &ResolvedPos,
    to: &ResolvedPos,
    depth: usize,
    schema: &Schema,
) -> Result<Fragment, ReplaceError> {
    let open_start = if from.depth() > depth {
        Some(joinable(schema, from, start, depth + 1)?)
    } else {
        None
    };
    let open_end = if to.depth() > depth {
        Some(joinable(schema, end, to, depth + 1)?)
    } else {
        None
    };

    let mut content = Vec::new();
    add_range(None, Some(from), depth, &mut content);

    match (&open_start, &open_end) {
        (Some(os), Some(oe)) if start.index(depth) == end.index(depth) => {
            check_join(schema, os, oe)?;
            let inner = replace_three_way(from, start, end, to, depth + 1, schema)?;
            add_node(close(schema, os, inner)?, &mut content);
        }
        _ => {
            if let Some(os) = &open_start {
                let inner = replace_two_way(from, start, depth + 1, schema)?;
                add_node(close(schema, os, inner)?, &mut content);
            }
            add_range(Some(start), Some(end), depth, &mut content);
            if let Some(oe) = &open_end {
                let inner = replace_two_way(end, to, depth + 1, schema)?;
                add_node(close(schema, oe, inner)?, &mut content);
            }
        }
    }

    add_range(Some(to), None, depth, &mut content);
    Ok(Fragment::from_vec(content))
}

fn prepare_slice_for_replace(
    slice: &Slice,
    along: &ResolvedPos,
) -> Result<(Node, ResolvedPos, ResolvedPos), DocError> {
    let extra = along.depth() - slice.open_start();
    let parent = along.node(extra);
    let mut node = parent.copy_content(slice.content().clone());
    for i in (0..extra).rev() {
        node = along.node(i).copy_content(Fragment::from_vec(vec![node]));
    }
    let start = ResolvedPos::resolve(&node, slice.open_start() + extra)?;
    let end_pos = node.content().size() - slice.open_end() - extra;
    let end = ResolvedPos::resolve(&node, end_pos)?;
    Ok((node, start, end))
}

fn replace_outer(
    from: &ResolvedPos,
    to: &ResolvedPos,
    slice: &Slice,
    depth: usize,
    schema: &Schema,
) -> Result<Node, ReplaceError> {
    let index = from.index(depth);
    let node = from.node(depth).clone();
    if index == to.index(depth) && depth < from.depth() - slice.open_start() {
        let inner = replace_outer(from, to, slice, depth + 1, schema)?;
        Ok(node.copy_content(node.content().replace_child(index, inner)))
    } else if slice.content().size() == 0 {
        let frag = replace_two_way(from, to, depth, schema)?;
        close(schema, &node, frag)
    } else if slice.open_start() == 0
        && slice.open_end() == 0
        && from.depth() == depth
        && to.depth() == depth
    {
        let parent = from.parent().clone();
        let content = parent.content();
        let merged = content
            .cut(0, from.parent_offset())
            .append(slice.content())
            .append(&content.cut(to.parent_offset(), content.size()));
        close(schema, &parent, merged)
    } else {
        let (_root, start, end) = prepare_slice_for_replace(slice, from).map_err(map_pos)?;
        let frag = replace_three_way(from, &start, &end, to, depth, schema)?;
        close(schema, &node, frag)
    }
}
