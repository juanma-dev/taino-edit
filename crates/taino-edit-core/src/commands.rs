//! Editing [`Command`]s — the standard vocabulary every WYSIWYG needs.
//!
//! A command follows the ProseMirror contract: called with no dispatch it
//! only *reports* whether it applies; called with a dispatch it performs the
//! change by handing it a [`Transaction`]. This makes commands composable
//! (try in order, stop at the first that applies) and keymap-bindable.

use crate::mark::MarkType;
use crate::node::Node;
use crate::selection::Selection;
use crate::state::{EditorState, Transaction};

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
