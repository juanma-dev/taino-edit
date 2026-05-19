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

/// Change the type (and attrs) of the block enclosing the selection — e.g.
/// paragraph → heading. Behaves on text, node and all selections (it acts on
/// the block at the selection's start).
pub fn set_block_type(node: &str, attrs: Attrs) -> Command {
    let node = node.to_string();
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((block, start, end)) = top_block(&rp) else {
            return false;
        };
        if block.is_text() || !block.node_type().is_block() {
            return false;
        }
        let Ok(new_block) = state.schema().node(
            &node,
            attrs.clone(),
            block.content().children().to_vec(),
            block.marks().to_vec(),
        ) else {
            return false;
        };
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            if tx
                .transform()
                .replace(start, end, node_slice(new_block), state.schema())
                .is_ok()
            {
                d(tx);
            }
        }
        true
    })
}

/// Wrap the block enclosing the selection in a new parent node.
pub fn wrap_in(node: &str, attrs: Attrs) -> Command {
    let node = node.to_string();
    Box::new(move |state, dispatch| {
        if state.schema().node_type(&node).is_none() {
            return false;
        }
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((_, start, end)) = top_block(&rp) else {
            return false;
        };
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

/// Collapse a range selection, or move an empty caret one position left.
pub fn caret_left(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    let target = if !sel.is_empty() {
        sel.from()
    } else if sel.from() > 0 {
        sel.from() - 1
    } else {
        return false;
    };
    if let Some(d) = dispatch {
        let mut tx = state.tr();
        tx.set_selection(Selection::caret(target));
        d(tx);
    }
    true
}

/// Collapse a range selection, or move an empty caret one position right.
pub fn caret_right(state: &EditorState, dispatch: Option<&mut Dispatch<'_>>) -> bool {
    let sel = state.selection();
    let max = state.doc().content().size();
    let target = if !sel.is_empty() {
        sel.to(state.doc())
    } else if sel.from() < max {
        sel.from() + 1
    } else {
        return false;
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
