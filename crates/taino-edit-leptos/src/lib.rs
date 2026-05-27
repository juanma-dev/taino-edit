//! `taino-edit-leptos` — the Leptos adapter for taino-edit.
//!
//! A thin reactive bridge: a [`TainoEditor`] component that mounts a
//! [`taino_edit_dom::EditorView`] inside its rendered `<div>` and reacts to
//! the editor state held in a `RwSignal<EditorState>`. The pure-Rust
//! `core`/`dom` layers do all the editing work; this crate is the glue that
//! makes them feel like a normal Leptos component.
//!
//! Browser events (`input`, `compositionstart`/`compositionend`, `paste`,
//! `selectionchange`) are wired automatically: each one runs the
//! corresponding `EditorView` method and folds the resulting transform —
//! or selection update — into the state signal.

#![warn(missing_docs, rust_2018_idioms)]

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// The [`schema!`](taino_edit_core::schema) builder macro.
pub use taino_edit_core::schema;
/// Re-exports of the most-used `taino-edit-core` items so adapter consumers
/// can stay on a single `use taino_edit_leptos::…` line.
#[doc(no_inline)]
pub use taino_edit_core::{
    base_keymap, delete_selection, join_backward, join_forward, lift, remove_mark, select_all,
    set_block_type, set_mark, split_block, toggle_mark, wrap_in, AttrSpec, AttrValue, Attrs,
    Command, Dispatch, DocError, DomSpec, EditorState, KeyPress, Keymap, Mark, MarkSpec, MarkType,
    Node, NodeSpec, NodeType, ResolvedPos, Schema, SchemaBuilder, Selection, Slice, Transaction,
    Transform,
};
/// Re-export the DOM-bridge surface that the Leptos adapter wraps.
#[doc(no_inline)]
pub use taino_edit_dom::{Decoration, EditorView, ViewAction, ViewDesc, ViewPlugin};

/// A Leptos component that renders an editor backed by a
/// `RwSignal<EditorState>`. Whenever the signal changes, the mounted DOM is
/// reconciled via [`EditorView::update`]; browser-side edits (typing, IME
/// commits, paste) feed back into the signal by applying the transforms
/// the DOM bridge produces.
///
/// ```ignore
/// use leptos::prelude::*;
/// use taino_edit_leptos::TainoEditor;
///
/// #[component]
/// fn App(state: RwSignal<EditorState>) -> impl IntoView {
///     view! { <TainoEditor state=state /> }
/// }
/// ```
#[component]
pub fn TainoEditor(
    /// The reactive editor state. The component reads the doc/schema from
    /// it on mount and applies an incremental DOM patch every time it
    /// changes; browser-side edits are committed back through it.
    state: RwSignal<EditorState>,
    /// Optional DOM-aware [`ViewPlugin`]s (e.g. `TableView` for table
    /// cell-drag-select + resize). Installed on the view at mount; the
    /// component wires pointer events to them and refreshes their
    /// decorations on every state change.
    #[prop(optional)]
    plugins: Vec<Box<dyn ViewPlugin>>,
) -> impl IntoView {
    let node_ref = NodeRef::<leptos::html::Div>::new();
    // `EditorView` + its event closures are `!Send + !Sync`. Keep them in
    // Leptos's local-storage slot so the (Send+Sync) effect closures can
    // reach them through a Copy handle without capturing the value itself.
    let runtime: StoredValue<Option<EditorRuntime>, LocalStorage> = StoredValue::new_local(None);
    // Plugins are `!Send` and consumed once at mount; park them in a
    // local slot the mount branch takes from.
    let plugins_slot: StoredValue<Option<Vec<Box<dyn ViewPlugin>>>, LocalStorage> =
        StoredValue::new_local(Some(plugins));

    Effect::new(move |_| {
        let snapshot = state.get();
        let Some(div) = node_ref.get() else {
            return;
        };
        runtime.update_value(|rt| match rt.as_mut() {
            Some(r) => {
                r.view.update(snapshot.doc().clone());
                // Re-sync the DOM selection from state — commands (toolbar
                // buttons, keymap, input-rules) move the doc selection
                // without touching the browser, and a no-op `read=write`
                // here is harmless. We guard against an echo from our own
                // `selectionchange` handler by setting a flag.
                if r.view.read_selection() != Some(snapshot.selection()) {
                    r.applying_selection.set(true);
                    let _ = r.view.set_selection(snapshot.selection());
                    r.applying_selection.set(false);
                }
                // Refresh plugin decorations (e.g. table cell-selection
                // highlight) for the current selection.
                r.view.refresh_view_decorations(Some(snapshot.selection()));
            }
            None => {
                let element: web_sys::Element = div.unchecked_into();
                let mut view = EditorView::mount(
                    snapshot.doc().clone(),
                    snapshot.schema().clone(),
                    element.clone(),
                );
                let plugins = plugins_slot
                    .try_update_value(|p| p.take())
                    .flatten()
                    .unwrap_or_default();
                view.set_view_plugins(plugins);
                view.refresh_view_decorations(Some(snapshot.selection()));
                let applying = std::rc::Rc::new(std::cell::Cell::new(false));
                let closures = wire_events(&element, runtime, state, applying.clone());
                *rt = Some(EditorRuntime {
                    view,
                    closures,
                    applying_selection: applying,
                });
            }
        });
    });

    on_cleanup(move || {
        runtime.set_value(None);
    });

    view! { <div node_ref=node_ref class="taino-editor"></div> }
}

/// What a mounted `TainoEditor` owns. Dropping this both drops the view
/// (frees the DOM-bound `EditorView`) and detaches every event listener.
struct EditorRuntime {
    view: EditorView,
    #[allow(dead_code)] // kept alive so the listeners they back stay attached.
    closures: Vec<EventCloser>,
    /// Set while the effect is pushing state's selection into the DOM, so
    /// the `selectionchange` listener can ignore the resulting echo.
    applying_selection: std::rc::Rc<std::cell::Cell<bool>>,
}

/// A `Closure` registered on a DOM target; on drop the listener is removed.
struct EventCloser {
    event: &'static str,
    target: web_sys::EventTarget,
    closure: Closure<dyn FnMut(web_sys::Event)>,
}

impl Drop for EventCloser {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.event, self.closure.as_ref().unchecked_ref());
    }
}

/// Attach the standard event listeners on `el`.
fn wire_events(
    el: &web_sys::Element,
    runtime: StoredValue<Option<EditorRuntime>, LocalStorage>,
    state: RwSignal<EditorState>,
    applying_selection: std::rc::Rc<std::cell::Cell<bool>>,
) -> Vec<EventCloser> {
    let target: web_sys::EventTarget = el.clone().into();
    let mut closers: Vec<EventCloser> = Vec::new();

    fn push_listener(
        closers: &mut Vec<EventCloser>,
        target: web_sys::EventTarget,
        event: &'static str,
        closure: Closure<dyn FnMut(web_sys::Event)>,
    ) {
        if target
            .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
            .is_ok()
        {
            closers.push(EventCloser {
                event,
                target,
                closure,
            });
        }
    }
    // Editor-element listeners. Scoped so `register` drops at the closing
    // brace, releasing the `&mut closers` borrow for the document-target
    // listener below.
    {
        let mut register = |event: &'static str, closure: Closure<dyn FnMut(web_sys::Event)>| {
            push_listener(&mut closers, target.clone(), event, closure);
        };

        // `input`: text typed or deleted in a text node.
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
            if let Some(Some(transform)) = with_view(runtime, |v| v.read_dom_changes()) {
                apply_transform(state, &transform);
            }
        });
        register("input", cb);

        // IME composition: suspend reads while composing, commit on end.
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
            with_view(runtime, |v| v.composition_start());
        });
        register("compositionstart", cb);

        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
            let transform = with_view(runtime, |v| {
                v.composition_end();
                v.read_dom_changes()
            })
            .flatten();
            if let Some(t) = transform {
                apply_transform(state, &t);
            }
        });
        register("compositionend", cb);

        // Paste: prefer Markdown (when advertised), then HTML, then plain
        // text. All three go through the schema-aware sanitisers in core.
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
            let Ok(clip) = ev.dyn_into::<web_sys::ClipboardEvent>() else {
                return;
            };
            clip.prevent_default();
            let Some(data) = clip.clipboard_data() else {
                return;
            };
            let md = data.get_data("text/markdown").unwrap_or_default();
            let html = data.get_data("text/html").unwrap_or_default();
            let text = data.get_data("text/plain").unwrap_or_default();
            let transform = with_view(runtime, |v| {
                if !md.is_empty() {
                    v.paste_markdown(&md)
                } else if !html.is_empty() {
                    v.paste_html(&html)
                } else if !text.is_empty() {
                    v.paste_text(&text)
                } else {
                    None
                }
            })
            .flatten();
            if let Some(t) = transform {
                apply_transform(state, &t);
            }
        });
        register("paste", cb);

        // Pointer events → view plugins (table cell-drag-select, resize).
        // Each fires `handle_view_event`; a returned action is applied to
        // state. No-op when no plugin claims the event.
        for kind in ["mousedown", "mousemove", "mouseup"] {
            let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
                if let Some(Some(action)) = with_view(runtime, |v| v.handle_view_event(&ev)) {
                    apply_view_action(state, action);
                }
            });
            register(kind, cb);
        }
    }

    // `selectionchange` only fires on `document`; mirror the browser
    // selection back into state so toolbar/keymap commands see the right
    // anchor/head. Drop self-induced echoes from the effect.
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        let doc_target: web_sys::EventTarget = doc.into();
        let applying = applying_selection.clone();
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
            if applying.get() {
                return;
            }
            let Some(Some(sel)) = with_view(runtime, |v| v.read_selection()) else {
                return;
            };
            let cur = state.with_untracked(|s| s.selection());
            if sel == cur {
                return;
            }
            state.update(|s| {
                let mut tx = s.tr();
                tx.set_selection(sel);
                tx.no_history();
                *s = s.apply(tx);
            });
        });
        push_listener(&mut closers, doc_target, "selectionchange", cb);
    }

    closers
}

fn with_view<R>(
    rt: StoredValue<Option<EditorRuntime>, LocalStorage>,
    f: impl FnOnce(&EditorView) -> R,
) -> Option<R> {
    rt.with_value(|r| r.as_ref().map(|r| f(&r.view)))
}

fn apply_transform(state: RwSignal<EditorState>, tr: &Transform) {
    state.update(|s| {
        let mut tx = s.tr();
        for step in tr.steps() {
            if tx.transform().step(step.clone(), s.schema()).is_err() {
                return; // bail without committing on schema rejection
            }
        }
        *s = s.apply(tx);
    });
}

/// Apply a [`ViewAction`] produced by a view plugin to the state signal.
fn apply_view_action(state: RwSignal<EditorState>, action: ViewAction) {
    match action {
        ViewAction::Select(sel) => {
            state.update(|s| {
                let mut tx = s.tr();
                tx.set_selection(sel);
                tx.no_history();
                *s = s.apply(tx);
            });
        }
        ViewAction::Command(cmd) => {
            let snapshot = state.get_untracked();
            let mut next = None;
            {
                let mut d = |tx: Transaction| next = Some(snapshot.apply(tx));
                cmd(&snapshot, Some(&mut d));
            }
            if let Some(n) = next {
                state.set(n);
            }
        }
    }
}
