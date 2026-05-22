//! Stateful editor plugins.
//!
//! A [`Plugin`] is a small unit of editor state that updates on every
//! [`Transaction`](crate::Transaction). Compared to an
//! [`Extension`](taino_edit_extensions::Extension), which only contributes
//! schema and keymap bindings, a `Plugin` carries its own typed state that
//! the editor folds forward as the document changes — think word counters,
//! spell-check state, autosave queues, future CRDT bridges.
//!
//! v0.2 ships the trait, the typed-erased registry baked into
//! [`EditorState`], and the [`PluginKey`] lookup.
//!
//! ## Observers, not drivers
//!
//! This trait is for **observer** plugins: [`Plugin::apply`] folds the
//! plugin's own state forward from each transaction
//! (`apply(tx, prev, state) -> state`) and deliberately *cannot* change
//! the document. That keeps the abstraction small and predictable.
//!
//! Components that need to *drive* the document — replace it wholesale,
//! like undo/redo — are a different category and intentionally do **not**
//! use this trait. The built-in `History` is the canonical example: it
//! rewrites the doc through a dedicated `HistoryIntent` short-circuit in
//! [`EditorState::apply`] and stays a first-class `EditorState` field. A
//! "HistoryPlugin" was evaluated and rejected (see `ROADMAP.md`,
//! v0.2.x backlog) — it would have bloated this trait with history-only
//! hooks for no gain.
//!
//! ```
//! use std::sync::Arc;
//! use taino_edit_core::{
//!     EditorState, NodeSpec, Plugin, PluginKey, PluginSet, SchemaBuilder,
//!     Transaction,
//! };
//!
//! /// Counts every doc-changing transaction.
//! struct WordCount;
//!
//! impl Plugin for WordCount {
//!     const NAME: &'static str = "word_count";
//!     type State = usize;
//!     fn init(&self, _state: &EditorState) -> usize { 0 }
//!     fn apply(&self, tx: &Transaction, _prev: &EditorState, n: usize) -> usize {
//!         if tx.doc_changed() { n + 1 } else { n }
//!     }
//! }
//!
//! const WC_KEY: PluginKey<WordCount> = PluginKey::new();
//!
//! let schema = SchemaBuilder::new()
//!     .node("doc",  NodeSpec { content: Some("text*".into()), ..Default::default() })
//!     .node("text", NodeSpec::default())
//!     .top_node("doc")
//!     .build()
//!     .unwrap();
//! let doc = schema.node("doc", Default::default(), vec![], vec![]).unwrap();
//! let plugins = PluginSet::new().with(WordCount);
//! let state = EditorState::with_plugins(doc, schema, plugins);
//! assert_eq!(state.plugin(WC_KEY), Some(&0));
//! ```

use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::state::{EditorState, Transaction};

/// A stateful editor plugin. Implementations carry no instance data beyond
/// configuration — the *state* the plugin manages lives in `EditorState`
/// and is fed back into [`Plugin::apply`] on each transaction.
pub trait Plugin: Send + Sync + 'static {
    /// A static identifier, unique within a `PluginSet`. The registry uses
    /// it as the storage key.
    const NAME: &'static str;

    /// The plugin's per-state value type. Cloned on each state update so
    /// `EditorState` stays inexpensive to fork.
    type State: Clone + Send + Sync + 'static;

    /// Compute the plugin's initial state, given the freshly-constructed
    /// editor state (doc + selection are already populated; other plugins
    /// may or may not be initialised yet, so don't peek at them here).
    fn init(&self, state: &EditorState) -> Self::State;

    /// Fold a transaction into the plugin's state. The default returns
    /// the previous state unchanged — handy for plugins that only read
    /// the doc.
    fn apply(
        &self,
        _tx: &Transaction,
        _prev_state: &EditorState,
        state: Self::State,
    ) -> Self::State {
        state
    }
}

/// A typed lookup handle for a plugin's state. Build one as
/// `const FOO_KEY: PluginKey<Foo> = PluginKey::new();` and pass it to
/// [`EditorState::plugin`].
pub struct PluginKey<P: Plugin>(PhantomData<fn() -> P>);

impl<P: Plugin> PluginKey<P> {
    /// A new key for plugin type `P`. The key is zero-sized; clone/copy
    /// freely.
    pub const fn new() -> Self {
        PluginKey(PhantomData)
    }

    /// The plugin's static name. Convenience accessor; you rarely need it.
    pub const fn name(&self) -> &'static str {
        P::NAME
    }
}

impl<P: Plugin> Default for PluginKey<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: Plugin> Clone for PluginKey<P> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<P: Plugin> Copy for PluginKey<P> {}

/// Object-safe shim so heterogeneous plugins can live in one collection.
pub(crate) trait StoredPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn init_erased(&self, state: &EditorState) -> Box<dyn Any + Send + Sync>;
    fn apply_erased(
        &self,
        tx: &Transaction,
        prev_state: &EditorState,
        state: &(dyn Any + Send + Sync),
    ) -> Box<dyn Any + Send + Sync>;
}

struct PluginAdapter<P: Plugin>(P);

impl<P: Plugin> StoredPlugin for PluginAdapter<P> {
    fn name(&self) -> &'static str {
        P::NAME
    }
    fn init_erased(&self, state: &EditorState) -> Box<dyn Any + Send + Sync> {
        Box::new(self.0.init(state))
    }
    fn apply_erased(
        &self,
        tx: &Transaction,
        prev_state: &EditorState,
        state: &(dyn Any + Send + Sync),
    ) -> Box<dyn Any + Send + Sync> {
        let typed: &P::State = state
            .downcast_ref::<P::State>()
            .expect("plugin state type mismatch — registry must be consistent");
        Box::new(self.0.apply(tx, prev_state, typed.clone()))
    }
}

/// Builder + container for the plugins an [`EditorState`] runs.
#[derive(Clone, Default)]
pub struct PluginSet {
    plugins: Vec<Arc<dyn StoredPlugin>>,
}

impl PluginSet {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `plugin` to the set. Plugins run in registration order on
    /// every transaction.
    pub fn with<P: Plugin>(mut self, plugin: P) -> Self {
        self.plugins.push(Arc::new(PluginAdapter(plugin)));
        self
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Whether the set has no plugins.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Arc<dyn StoredPlugin>> {
        self.plugins.iter()
    }
}

impl std::fmt::Debug for PluginSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginSet")
            .field(
                "plugins",
                &self.plugins.iter().map(|p| p.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// The per-state map of plugin states. Stored inside `EditorState`.
#[derive(Clone, Default)]
pub(crate) struct PluginStates {
    states: Vec<(&'static str, Arc<dyn Any + Send + Sync>)>,
    set: PluginSet,
}

impl PluginStates {
    pub(crate) fn from_set(set: PluginSet, state: &EditorState) -> Self {
        let states = set
            .iter()
            .map(|p| (p.name(), Arc::from(p.init_erased(state))))
            .collect();
        PluginStates { states, set }
    }

    pub(crate) fn apply(&self, tx: &Transaction, prev_state: &EditorState) -> Self {
        let new_states: Vec<(&'static str, Arc<dyn Any + Send + Sync>)> = self
            .set
            .iter()
            .zip(self.states.iter())
            .map(|(plugin, (name, state))| {
                let next = plugin.apply_erased(tx, prev_state, state.as_ref());
                (*name, Arc::from(next))
            })
            .collect();
        PluginStates {
            states: new_states,
            set: self.set.clone(),
        }
    }

    pub(crate) fn get<P: Plugin>(&self) -> Option<&P::State> {
        self.states
            .iter()
            .find(|(n, _)| *n == P::NAME)
            .and_then(|(_, s)| s.downcast_ref::<P::State>())
    }
}

impl std::fmt::Debug for PluginStates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginStates")
            .field(
                "plugins",
                &self.states.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            )
            .finish()
    }
}
