//! Editing [`Command`]s — the standard vocabulary every WYSIWYG needs.
//!
//! A command follows the ProseMirror contract: called with no dispatch it
//! only *reports* whether it applies; called with a dispatch it performs the
//! change by handing it a [`Transaction`]. This makes commands composable
//! (try in order, stop at the first that applies) and keymap-bindable.

use crate::attrs::Attrs;
use crate::fragment::Fragment;
use crate::mark::MarkType;
use crate::node::Node;
use crate::pos::ResolvedPos;
use crate::schema::Schema;
use crate::selection::Selection;
use crate::slice::Slice;
use crate::state::{EditorState, Transaction};
use crate::step::ReplaceAroundStep;

/// A dispatch sink for a performed command.
pub type Dispatch<'a> = dyn FnMut(Transaction) + 'a;

/// An editing command. Returns whether it is applicable; performs the change
/// only when a `dispatch` is supplied.
pub type Command = Box<dyn Fn(&EditorState, Option<&mut Dispatch<'_>>) -> bool>;

/// Try `commands` in order; run (and report `true` for) the first that
/// applies.
pub fn chain(commands: Vec<Command>) -> Command {
    Box::new(move |state, mut dispatch| {
        for cmd in &commands {
            // Probe without dispatching first so we never half-apply.
            if cmd(state, None) {
                if let Some(d) = dispatch.as_deref_mut() {
                    cmd(state, Some(d));
                }
                return true;
            }
        }
        false
    })
}

/// Select the whole document.
pub fn select_all(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        tx.set_selection(Selection::All);
        d(tx);
    }
    true
}

/// Delete the current (non-empty) selection.
pub fn delete_selection(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    let (from, to) = (sel.from(), sel.to(state.doc()));
    if from == to {
        return false;
    }
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        if tx.transform().delete(from, to, state.schema()).is_ok() {
            tx.set_selection(Selection::caret(from));
            d(tx);
        }
    }
    true
}

/// Whether every inline node in `from..to` carries `mark` (and there is at
/// least one inline node).
fn range_fully_marked(doc: &Node, from: usize, to: usize, mark: &MarkType) -> bool {
    let Ok(slice) = doc.slice(from, to) else {
        return false;
    };
    let mut seen = false;
    let mut all = true;
    fn walk(n: &Node, mark: &MarkType, seen: &mut bool, all: &mut bool) {
        if n.is_inline() {
            *seen = true;
            if !n.marks().iter().any(|m| m.mark_type() == mark) {
                *all = false;
            }
        }
        for c in n.content().iter() {
            walk(c, mark, seen, all);
        }
    }
    for n in slice.content().iter() {
        walk(n, mark, &mut seen, &mut all);
    }
    seen && all
}

fn marked_command(mark: MarkType, force_add: Option<bool>) -> Command {
    Box::new(move |state, dispatch| {
        let sel = state.selection();
        let (from, to) = (sel.from(), sel.to(state.doc()));
        if from >= to {
            return false; // v0.1: no stored-marks on an empty caret
        }
        let add = match force_add {
            Some(v) => v,
            None => !range_fully_marked(state.doc(), from, to, &mark),
        };
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let m = mark.create(Default::default());
            let r = if add {
                tx.transform().add_mark(from, to, m, state.schema())
            } else {
                tx.transform().remove_mark(from, to, m, state.schema())
            };
            if r.is_ok() {
                d(tx);
            }
        }
        true
    })
}

/// Toggle `mark` over the selection (add if any covered text lacks it, else
/// remove).
pub fn toggle_mark(mark: MarkType) -> Command {
    marked_command(mark, None)
}

/// Add `mark` over the selection.
pub fn set_mark(mark: MarkType) -> Command {
    marked_command(mark, Some(true))
}

/// Remove `mark` over the selection.
pub fn remove_mark(mark: MarkType) -> Command {
    marked_command(mark, Some(false))
}

fn node_slice(node: Node) -> Slice {
    Slice::new(Fragment::from_node(node), 0, 0)
}

/// The top-level (depth-1) block enclosing `pos`, with its before/after
/// positions. `None` at the very top.
fn top_block(rp: &ResolvedPos) -> Option<(Node, usize, usize)> {
    if rp.depth() == 0 {
        return None;
    }
    Some((rp.node(1).clone(), rp.before(1), rp.after(1)))
}

/// Every top-level (depth-1) block whose extent intersects the selection range
/// `[from, to]`, each as `(node, before, after)` in document order.
///
/// Block-level commands (`set_block_type`, alignment, lists, blockquote) use
/// this so they affect **all** blocks the selection touches, not just the one
/// at its start. For an empty/boundary caret it falls back to the single
/// enclosing block, preserving caret behavior.
pub fn top_blocks_in_range(doc: &Node, from: usize, to: usize) -> Vec<(Node, usize, usize)> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    for child in doc.content().iter() {
        let start = pos;
        let after = start + child.node_size();
        if from < after && to > start {
            out.push((child.clone(), start, after));
        }
        pos = after;
    }
    if out.is_empty() {
        if let Ok(rp) = ResolvedPos::resolve(doc, from) {
            if let Some(b) = top_block(&rp) {
                out.push(b);
            }
        }
    }
    out
}

/// Change the type (and attrs) of every block the selection touches — e.g.
/// paragraph → heading across several selected paragraphs. Each convertible
/// textblock keeps its inline content and marks. Re-typing preserves block
/// sizes, so the collected positions stay valid across the whole transaction.
pub fn set_block_type(node: &str, attrs: Attrs) -> Command {
    let node = node.to_string();
    Box::new(move |state, dispatch| {
        let sel = state.selection();
        let blocks = top_blocks_in_range(state.doc(), sel.from(), sel.to(state.doc()));
        // Build a replacement for each convertible block (skip non-blocks and
        // any whose content the target type can't hold).
        let mut targets: Vec<(usize, usize, Node)> = Vec::new();
        for (block, start, end) in blocks {
            if block.is_text() || !block.node_type().is_block() {
                continue;
            }
            if let Ok(new_block) = state.schema().node(
                &node,
                attrs.clone(),
                block.content().children().to_vec(),
                block.marks().to_vec(),
            ) {
                targets.push((start, end, new_block));
            }
        }
        if targets.is_empty() {
            return false;
        }
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let mut any = false;
            for (start, end, new_block) in targets {
                if tx
                    .transform()
                    .replace(start, end, node_slice(new_block), state.schema())
                    .is_ok()
                {
                    any = true;
                }
            }
            if any {
                d(tx);
            }
        }
        true
    })
}

/// Wrap every block the selection touches in a single new parent node — e.g.
/// three selected paragraphs become one blockquote containing all three.
pub fn wrap_in(node: &str, attrs: Attrs) -> Command {
    let node = node.to_string();
    Box::new(move |state, dispatch| {
        if state.schema().node_type(&node).is_none() {
            return false;
        }
        let sel = state.selection();
        let blocks = top_blocks_in_range(state.doc(), sel.from(), sel.to(state.doc()));
        let (Some((_, start, _)), Some((_, _, end))) = (blocks.first(), blocks.last()) else {
            return false;
        };
        let (start, end) = (*start, *end);
        let Ok(wrapper) = state
            .schema()
            .create_node(&node, attrs.clone(), vec![], vec![])
        else {
            return false;
        };
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let step = ReplaceAroundStep::new(start, end, start, end, node_slice(wrapper), 1);
            if tx.transform().step(Box::new(step), state.schema()).is_ok() {
                d(tx);
            }
        }
        true
    })
}

/// Lift the textblock out of its immediate single-child wrapper (e.g. a
/// paragraph out of a one-paragraph blockquote). v0.1 handles the
/// single-child case; richer lifting is a v0.2 refinement.
pub fn lift(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
        return false;
    };
    let d = rp.depth();
    if d < 2 {
        return false;
    }
    let wrapper_depth = d - 1;
    let wrapper = rp.node(wrapper_depth);
    if wrapper.child_count() != 1 {
        return false;
    }
    let start = rp.before(wrapper_depth);
    let end = rp.after(wrapper_depth);
    let content = Slice::new(wrapper.content().clone(), 0, 0);
    if let Some(disp) = dispatch {
        let mut tx = state.tr();
        if tx
            .transform()
            .replace(start, end, content, state.schema())
            .is_ok()
        {
            disp(tx);
        }
    }
    true
}

/// Split the textblock at an empty caret (Enter).
pub fn split_block(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    if !sel.is_empty() {
        return false;
    }
    let pos = sel.from();
    let Ok(rp) = ResolvedPos::resolve(state.doc(), pos) else {
        return false;
    };
    if rp.depth() == 0 || !rp.parent().node_type().is_block() {
        return false;
    }
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        if tx.transform().split(pos, state.schema()).is_ok() {
            tx.set_selection(Selection::caret(pos + 2));
            d(tx);
        }
    }
    true
}

/// Delete the character before an empty caret within its textblock.
pub fn delete_backward(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    if !sel.is_empty() {
        return false;
    }
    let pos = sel.from();
    let Ok(rp) = ResolvedPos::resolve(state.doc(), pos) else {
        return false;
    };
    if rp.depth() == 0 || rp.parent_offset() == 0 {
        return false;
    }
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        if tx.transform().delete(pos - 1, pos, state.schema()).is_ok() {
            tx.set_selection(Selection::caret(pos - 1));
            d(tx);
        }
    }
    true
}

/// Delete the character after an empty caret within its textblock.
pub fn delete_forward(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    if !sel.is_empty() {
        return false;
    }
    let pos = sel.from();
    let Ok(rp) = ResolvedPos::resolve(state.doc(), pos) else {
        return false;
    };
    if rp.depth() == 0 || rp.parent_offset() == rp.parent().content().size() {
        return false;
    }
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        if tx.transform().delete(pos, pos + 1, state.schema()).is_ok() {
            d(tx);
        }
    }
    true
}

/// Whether `rp` sits inside a textblock — a block whose content allows
/// inline (text) children. Only such positions are valid text carets; the
/// boundaries *between* blocks (e.g. after a `</p>` but still inside a
/// `<li>`) are not.
fn pos_in_textblock(rp: &ResolvedPos, schema: &Schema) -> bool {
    if rp.depth() == 0 {
        return false;
    }
    let parent = rp.parent();
    if !parent.node_type().is_block() {
        return false;
    }
    let Some(text_type) = schema.node_type("text") else {
        return false;
    };
    schema
        .content_match(parent.node_type().id())
        .match_type(text_type.id())
        .is_some()
}

/// Scan outward from `from` (exclusive) in the given direction for the next
/// position that is a valid text caret. Returns `None` at the document edge
/// — so a caret already at the last text position of the document doesn't
/// drift into a structural boundary.
fn next_text_caret(doc: &Node, schema: &Schema, from: usize, forward: bool) -> Option<usize> {
    let max = doc.content().size();
    let mut p = from;
    loop {
        if forward {
            if p >= max {
                return None;
            }
            p += 1;
        } else if p == 0 {
            return None;
        } else {
            p -= 1;
        }
        if let Ok(rp) = ResolvedPos::resolve(doc, p) {
            if pos_in_textblock(&rp, schema) {
                return Some(p);
            }
        }
    }
}

/// Collapse a range selection, or move an empty caret to the previous valid
/// text position (skipping structural boundaries between blocks).
pub fn caret_left(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    let target = if !sel.is_empty() {
        sel.from()
    } else {
        match next_text_caret(state.doc(), state.schema(), sel.from(), false) {
            Some(p) => p,
            None => return false,
        }
    };
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        tx.set_selection(Selection::caret(target));
        d(tx);
    }
    true
}

/// Collapse a range selection, or move an empty caret to the next valid text
/// position (skipping structural boundaries between blocks).
pub fn caret_right(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    let target = if !sel.is_empty() {
        sel.to(state.doc())
    } else {
        match next_text_caret(state.doc(), state.schema(), sel.from(), true) {
            Some(p) => p,
            None => return false,
        }
    };
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        tx.set_selection(Selection::caret(target));
        d(tx);
    }
    true
}

/// Move the caret to the start of its textblock (Home).
pub fn caret_line_start(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
        return false;
    };
    if rp.depth() == 0 {
        return false;
    }
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        tx.set_selection(Selection::caret(rp.start(rp.depth())));
        d(tx);
    }
    true
}

/// Move the caret to the end of its textblock (End).
pub fn caret_line_end(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
        return false;
    };
    if rp.depth() == 0 {
        return false;
    }
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        tx.set_selection(Selection::caret(rp.end(rp.depth())));
        d(tx);
    }
    true
}

fn find_last_textblock(node: &Node, start_pos: usize) -> Option<(Node, usize)> {
    if node.is_text() {
        return None;
    }
    if node.is_block() && (node.child_count() == 0 || node.child(0).is_inline()) {
        return Some((node.clone(), start_pos));
    }
    let mut pos = start_pos + 1;
    for i in 0..node.child_count() {
        let child = node.child(i);
        if i == node.child_count() - 1 {
            return find_last_textblock(child, pos);
        }
        pos += child.node_size();
    }
    None
}

/// At the start of a block with a preceding sibling, join it onto that
/// sibling (Backspace at block start).
pub fn join_backward(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    if !sel.is_empty() {
        return false;
    }
    let Ok(rp) = ResolvedPos::resolve(state.doc(), sel.from()) else {
        return false;
    };
    let d = rp.depth();
    if d == 0 || rp.parent_offset() != 0 || rp.index(d - 1) == 0 {
        return false;
    }

    let parent = rp.node(d - 1);
    let prev_idx = rp.index(d - 1) - 1;
    let prev_sibling = parent.child(prev_idx);
    let prev_sibling_start = rp.before(d) - prev_sibling.node_size();

    // Check if the preceding sibling is a structural wrapper node that is not a textblock.
    if prev_sibling.is_block() && (prev_sibling.child_count() > 0 && !prev_sibling.child(0).is_inline()) {
        if let Some((last_p, last_p_start)) = find_last_textblock(prev_sibling, prev_sibling_start) {
            let target_end = last_p_start + 1 + last_p.content().size();
            let current_end = rp.after(d);
            if let Some(disp) = dispatch {
                let mut tx = state.tr();
                let Ok(rp_last) = ResolvedPos::resolve(state.doc(), last_p_start + 1) else {
                    return false;
                };
                let mut current_node = rp.node(d).clone();
                for depth in (1..rp_last.depth()).rev() {
                    let ancestor = rp_last.node(depth);
                    let Ok(wrapped) = state.schema().create_node(
                        ancestor.node_type().name(),
                        ancestor.attrs().clone(),
                        vec![current_node],
                        ancestor.marks().to_vec(),
                    ) else {
                        return false;
                    };
                    current_node = wrapped;
                }
                let slice = Slice::new(Fragment::from_node(current_node), rp_last.depth(), 0);
                if tx
                    .transform()
                    .replace(target_end, current_end, slice, state.schema())
                    .is_ok()
                {
                    tx.set_selection(Selection::caret(target_end));
                    disp(tx);
                }
            }
            return true;
        }
    }

    let before = rp.before(d);
    if let Some(disp) = dispatch {
        let mut tx = state.tr();
        if tx
            .transform()
            .delete(before - 1, before + 1, state.schema())
            .is_ok()
        {
            tx.set_selection(Selection::caret(before - 1));
            disp(tx);
        }
    }
    true
}

/// At the end of a block with a following sibling, join that sibling onto it
/// (Delete at block end).
pub fn join_forward(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    if !sel.is_empty() {
        return false;
    }
    let Ok(rp) = ResolvedPos::resolve(state.doc(), sel.from()) else {
        return false;
    };
    let d = rp.depth();
    if d == 0 || rp.parent_offset() != rp.parent().content().size() {
        return false;
    }
    let parent = rp.node(d - 1);
    if rp.index(d - 1) + 1 >= parent.child_count() {
        return false;
    }
    let after = rp.after(d);
    if let Some(disp) = dispatch {
        let mut tx = state.tr();
        if tx
            .transform()
            .delete(after - 1, after + 1, state.schema())
            .is_ok()
        {
            disp(tx);
        }
    }
    true
}
