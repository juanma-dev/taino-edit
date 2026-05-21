//! v0.2 — `Plugin` + `PluginKey` + typed-state registry.

use taino_edit_core::{
    EditorState, NodeSpec, Plugin, PluginKey, PluginSet, Schema, SchemaBuilder, Selection,
    Transaction,
};

fn schema() -> Schema {
    SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("paragraph+".into()),
                ..Default::default()
            },
        )
        .node(
            "paragraph",
            NodeSpec {
                content: Some("text*".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        )
        .top_node("doc")
        .build()
        .unwrap()
}

fn make_doc(s: &Schema, text: &str) -> taino_edit_core::Node {
    let t = s.text(text, vec![]).unwrap();
    let p = s
        .node("paragraph", Default::default(), vec![t], vec![])
        .unwrap();
    s.node("doc", Default::default(), vec![p], vec![]).unwrap()
}

// ---- Counter plugin: increments on every doc-changing transaction. -------

struct Counter;

impl Plugin for Counter {
    const NAME: &'static str = "counter";
    type State = usize;
    fn init(&self, _state: &EditorState) -> usize {
        0
    }
    fn apply(&self, tx: &Transaction, _prev: &EditorState, state: usize) -> usize {
        if tx.doc_changed() {
            state + 1
        } else {
            state
        }
    }
}

const COUNTER_KEY: PluginKey<Counter> = PluginKey::new();

// ---- DocLen plugin: caches the text-content length each tick. ------------

struct DocLen;

impl Plugin for DocLen {
    const NAME: &'static str = "doc_len";
    type State = usize;
    fn init(&self, state: &EditorState) -> usize {
        state.doc().text_content().chars().count()
    }
    fn apply(&self, _tx: &Transaction, prev: &EditorState, _state: usize) -> usize {
        // Apply runs after the doc was mutated — `prev.doc()` is still
        // the pre-tx doc, so the caller's view of state.apply observes
        // the new doc-length only after the next state is constructed.
        // The next state then re-runs init on the new doc by virtue of
        // PluginStates::apply calling Plugin::apply with the NEW doc
        // available via... hmm actually `prev_state` IS the new state
        // post-doc here. Let me just recompute from prev.
        prev.doc().text_content().chars().count()
    }
}

const DOC_LEN_KEY: PluginKey<DocLen> = PluginKey::new();

#[test]
fn plugin_key_is_zst() {
    assert_eq!(std::mem::size_of::<PluginKey<Counter>>(), 0);
}

#[test]
fn plugin_set_is_empty_by_default() {
    let p = PluginSet::new();
    assert!(p.is_empty());
    assert_eq!(p.len(), 0);
}

#[test]
fn plugin_set_with_appends() {
    let p = PluginSet::new().with(Counter).with(DocLen);
    assert_eq!(p.len(), 2);
}

#[test]
fn editor_state_initialises_plugins() {
    let s = schema();
    let doc = make_doc(&s, "hello");
    let plugins = PluginSet::new().with(Counter).with(DocLen);
    let state = EditorState::with_plugins(doc, s, plugins);

    assert_eq!(state.plugin(COUNTER_KEY), Some(&0));
    assert_eq!(state.plugin(DOC_LEN_KEY), Some(&5));
}

#[test]
fn plugin_apply_runs_on_every_doc_changing_transaction() {
    let s = schema();
    let doc = make_doc(&s, "hi");
    let plugins = PluginSet::new().with(Counter);
    let mut state = EditorState::with_plugins(doc, s.clone(), plugins);

    // Selection-only transaction does NOT touch counter.
    let mut tx = state.tr();
    tx.set_selection(Selection::caret(2));
    state = state.apply(tx);
    assert_eq!(state.plugin(COUNTER_KEY), Some(&0));

    // A doc-changing transaction DOES bump counter.
    let mut tx = state.tr();
    let new_text = s.text("X", vec![]).unwrap();
    let slice = taino_edit_core::Slice::new(taino_edit_core::Fragment::from_node(new_text), 0, 0);
    tx.transform().replace(1, 1, slice, &s).unwrap();
    state = state.apply(tx);
    assert_eq!(state.plugin(COUNTER_KEY), Some(&1));
}

#[test]
fn plugin_state_is_unset_for_unregistered_plugins() {
    let s = schema();
    let doc = make_doc(&s, "hi");
    // Counter registered but not DocLen.
    let state = EditorState::with_plugins(doc, s, PluginSet::new().with(Counter));
    assert!(state.plugin(DOC_LEN_KEY).is_none());
}

#[test]
fn back_compat_editor_state_new_works_without_plugins() {
    // The pre-existing `EditorState::new(doc, schema)` constructor must
    // keep working for callers that don't care about plugins.
    let s = schema();
    let doc = make_doc(&s, "hi");
    let state = EditorState::new(doc, s);
    assert!(state.plugin(COUNTER_KEY).is_none());
    assert_eq!(state.doc().text_content(), "hi");
}
