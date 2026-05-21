//! `history` — undo/redo commands bound to `Mod-z` and `Mod-Shift-z`. The
//! core history machinery already lives on [`taino_edit_core::EditorState`];
//! this extension just wires it into the standard keymap via a transaction
//! tagged with a [`HistoryIntent`].

use taino_edit_core::{Command, EditorState, HistoryIntent, Schema};

use crate::Extension;

/// The undo command. Applicable iff the undo stack is non-empty.
pub fn undo_command() -> Command {
    Box::new(|state: &EditorState, dispatch| {
        if state.history().undo_depth() == 0 {
            return false;
        }
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            tx.set_history_intent(HistoryIntent::Undo);
            d(tx);
        }
        true
    })
}

/// The redo command. Applicable iff the undone stack is non-empty.
pub fn redo_command() -> Command {
    Box::new(|state: &EditorState, dispatch| {
        if state.history().redo_depth() == 0 {
            return false;
        }
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            tx.set_history_intent(HistoryIntent::Redo);
            d(tx);
        }
        true
    })
}

/// The history extension. Binds `Mod-z` → undo and `Mod-Shift-z` → redo.
pub struct History;

impl Extension for History {
    fn name(&self) -> &str {
        "history"
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        vec![
            ("Mod-z".to_string(), undo_command()),
            ("Mod-Shift-z".to_string(), redo_command()),
        ]
    }
}
