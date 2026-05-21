//! Editor state: [`EditorState`] (document + selection + schema + history),
//! [`Transaction`] (a [`Transform`] that also tracks selection and history
//! intent), and a bounded undo/redo [`History`].
//!
//! v0.1 ships history as the one built-in stateful component rather than a
//! general typed-plugin registry; the plugin-registry generalisation is a
//! v0.2 item (see ROADMAP). Undo/redo is exact for linear single-user
//! editing, which is the v0.1 target.

use crate::node::Node;
use crate::plugin::{Plugin, PluginKey, PluginSet, PluginStates};
use crate::schema::Schema;
use crate::selection::Selection;
use crate::step::{Step, StepError};
use crate::transform::Transform;

const DEFAULT_HISTORY_DEPTH: usize = 100;

fn apply_steps(doc: &Node, steps: &[Box<dyn Step>], schema: &Schema) -> Result<Node, StepError> {
    let mut cur = doc.clone();
    for s in steps {
        cur = s.apply(&cur, schema)?;
    }
    Ok(cur)
}

#[derive(Debug, Clone)]
struct HistEntry {
    /// Applied to the *new* doc, reproduce the *old* doc.
    undo: Vec<Box<dyn Step>>,
    /// Applied to the *old* doc, reproduce the *new* doc.
    redo: Vec<Box<dyn Step>>,
    selection_before: Selection,
    selection_after: Selection,
}

/// A bounded, linear undo/redo stack.
#[derive(Debug, Clone)]
pub struct History {
    done: Vec<HistEntry>,
    undone: Vec<HistEntry>,
    depth: usize,
}

impl Default for History {
    fn default() -> Self {
        History {
            done: Vec::new(),
            undone: Vec::new(),
            depth: DEFAULT_HISTORY_DEPTH,
        }
    }
}

impl History {
    /// A history bounded to `depth` undoable groups.
    pub fn with_depth(depth: usize) -> Self {
        History {
            depth,
            ..Default::default()
        }
    }

    /// Number of undoable groups.
    pub fn undo_depth(&self) -> usize {
        self.done.len()
    }

    /// Number of redoable groups.
    pub fn redo_depth(&self) -> usize {
        self.undone.len()
    }

    fn record(&mut self, mut entry: HistEntry, join: bool) {
        self.undone.clear();
        if join {
            if let Some(prev) = self.done.last_mut() {
                // Combined undo: newest→mid (entry.undo) then mid→old
                // (prev.undo); combined redo: old→mid then mid→new.
                let mut undo = std::mem::take(&mut entry.undo);
                undo.extend(std::mem::take(&mut prev.undo));
                prev.undo = undo;
                prev.redo.extend(std::mem::take(&mut entry.redo));
                prev.selection_after = entry.selection_after;
                return;
            }
        }
        self.done.push(entry);
        if self.done.len() > self.depth {
            self.done.remove(0);
        }
    }
}

/// The complete editor state.
#[derive(Debug, Clone)]
pub struct EditorState {
    doc: Node,
    selection: Selection,
    schema: Schema,
    history: History,
    plugins: PluginStates,
}

impl EditorState {
    /// A fresh state for `doc`, caret at the document start.
    pub fn new(doc: Node, schema: Schema) -> Self {
        Self::with_plugins(doc, schema, PluginSet::new())
    }

    /// A fresh state pre-loaded with a [`PluginSet`]. Each plugin's
    /// [`Plugin::init`] is called with the (partially-initialised) state,
    /// so plugins can derive their initial value from the doc.
    pub fn with_plugins(doc: Node, schema: Schema, plugins: PluginSet) -> Self {
        let seed = EditorState {
            doc,
            selection: Selection::caret(0),
            schema,
            history: History::default(),
            plugins: PluginStates::default(),
        };
        let plugin_states = PluginStates::from_set(plugins, &seed);
        EditorState {
            plugins: plugin_states,
            ..seed
        }
    }

    /// Use a custom history depth.
    pub fn with_history(mut self, history: History) -> Self {
        self.history = history;
        self
    }

    /// Borrow this state's plugin value (returns `None` if no plugin
    /// of type `P` was registered when the state was built).
    pub fn plugin<P: Plugin>(&self, _key: PluginKey<P>) -> Option<&P::State> {
        self.plugins.get::<P>()
    }

    /// The current document.
    pub fn doc(&self) -> &Node {
        &self.doc
    }
    /// The current selection.
    pub fn selection(&self) -> Selection {
        self.selection
    }
    /// The schema.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }
    /// Undoable / redoable depth.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Begin a transaction from the current state.
    pub fn tr(&self) -> Transaction {
        Transaction {
            tr: Transform::new(self.doc.clone()),
            selection: self.selection,
            selection_set: false,
            add_to_history: true,
            join: false,
            history_intent: None,
        }
    }

    /// Apply `tx`, returning the next state. Selection is mapped through the
    /// transaction unless the transaction set one explicitly; a changing,
    /// history-tracked transaction records an undo group. Transactions
    /// carrying a [`HistoryIntent`] resolve to [`undo`](Self::undo) /
    /// [`redo`](Self::redo) on this state (and never push another history
    /// entry); if the stack is empty the current state is returned unchanged.
    pub fn apply(&self, tx: Transaction) -> EditorState {
        if let Some(intent) = tx.history_intent {
            return match intent {
                HistoryIntent::Undo => self.undo().unwrap_or_else(|| self.clone()),
                HistoryIntent::Redo => self.redo().unwrap_or_else(|| self.clone()),
            };
        }
        let new_doc = tx.tr.doc().clone();
        let selection = if tx.selection_set {
            tx.selection
        } else {
            self.selection.map(&new_doc, tx.tr.mapping())
        };

        let mut history = self.history.clone();
        if tx.add_to_history && tx.tr.doc_changed() {
            if let Ok(undo) = tx.tr.invert_steps() {
                let redo: Vec<Box<dyn Step>> = tx.tr.steps().to_vec();
                history.record(
                    HistEntry {
                        undo,
                        redo,
                        selection_before: self.selection,
                        selection_after: selection,
                    },
                    tx.join,
                );
            }
        }

        // Fold plugin states forward against the just-applied tx.
        let plugins = self.plugins.apply(&tx, self);

        EditorState {
            doc: new_doc,
            selection,
            schema: self.schema.clone(),
            history,
            plugins,
        }
    }

    /// Undo the most recent group, or `None` if nothing to undo.
    pub fn undo(&self) -> Option<EditorState> {
        let entry = self.history.done.last()?.clone();
        let doc = apply_steps(&self.doc, &entry.undo, &self.schema).ok()?;
        let mut history = self.history.clone();
        history.done.pop();
        history.undone.push(entry.clone());
        Some(EditorState {
            doc,
            selection: entry.selection_before,
            schema: self.schema.clone(),
            history,
            plugins: self.plugins.clone(),
        })
    }

    /// Redo the most recently undone group, or `None`.
    pub fn redo(&self) -> Option<EditorState> {
        let entry = self.history.undone.last()?.clone();
        let doc = apply_steps(&self.doc, &entry.redo, &self.schema).ok()?;
        let mut history = self.history.clone();
        history.undone.pop();
        history.done.push(entry.clone());
        Some(EditorState {
            doc,
            selection: entry.selection_after,
            schema: self.schema.clone(),
            history,
            plugins: self.plugins.clone(),
        })
    }
}

/// Whether a transaction is asking the state to walk its undo/redo stack
/// instead of applying steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryIntent {
    /// Undo the most recent done group.
    Undo,
    /// Redo the most recently undone group.
    Redo,
}

/// A pending change: a [`Transform`] plus selection and history intent.
#[derive(Debug, Clone)]
pub struct Transaction {
    tr: Transform,
    selection: Selection,
    selection_set: bool,
    add_to_history: bool,
    join: bool,
    history_intent: Option<HistoryIntent>,
}

impl Transaction {
    /// The (in-progress) transformed document.
    pub fn doc(&self) -> &Node {
        self.tr.doc()
    }

    /// Mutable access to the underlying transform (apply steps via its
    /// helpers, e.g. `tx.transform().replace(..)`).
    pub fn transform(&mut self) -> &mut Transform {
        &mut self.tr
    }

    /// Explicitly set the selection for the resulting state.
    pub fn set_selection(&mut self, selection: Selection) -> &mut Self {
        self.selection = selection;
        self.selection_set = true;
        self
    }

    /// Exclude this transaction from undo history.
    pub fn no_history(&mut self) -> &mut Self {
        self.add_to_history = false;
        self
    }

    /// Merge this transaction into the previous undo group (e.g. continued
    /// typing). Grouping is caller-driven in v0.1.
    pub fn join_history(&mut self) -> &mut Self {
        self.join = true;
        self
    }

    /// Whether the document was changed.
    pub fn doc_changed(&self) -> bool {
        self.tr.doc_changed()
    }

    /// Tag this transaction so [`EditorState::apply`] walks the undo/redo
    /// stack instead of applying steps. Used by the History extension's
    /// commands to dispatch through the normal `Command`/`Dispatch` pipeline.
    pub fn set_history_intent(&mut self, intent: HistoryIntent) -> &mut Self {
        self.history_intent = Some(intent);
        self
    }

    /// The history intent on this transaction, if any.
    pub fn history_intent(&self) -> Option<HistoryIntent> {
        self.history_intent
    }
}
