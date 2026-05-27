//! `taino-edit-dioxus` — the Dioxus adapter for taino-edit.
//!
//! Mirrors `taino-edit-leptos`: a [`TainoEditor`] component takes a
//! [`Signal<EditorState>`] and mounts a [`taino_edit_dom::EditorView`]
//! inside its rendered `<div>`, reconciling the DOM on every signal
//! change and folding browser-side edits back into the signal.
//!
//! Browser events (`input`, `compositionstart`/`compositionend`, `paste`,
//! pointer `mousedown`/`mousemove`/`mouseup`, `selectionchange`) are wired
//! with the same raw `web-sys` listeners the Leptos adapter uses — they are
//! registered on the mounted element (and, for `selectionchange`, on
//! `document`) and kept alive in the component's runtime slot. Optional
//! [`ViewPlugin`]s (e.g. `TableView`) can be installed via the [`ViewPlugins`]
//! prop, giving full event- and plugin-wiring parity with `taino-edit-leptos`.

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(missing_docs, rust_2018_idioms)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// The [`schema!`](taino_edit_core::schema) builder macro.
pub use taino_edit_core::schema;
/// Re-export the core types adapter consumers reach for most.
#[doc(no_inline)]
pub use taino_edit_core::{
    base_keymap, lift, remove_mark, select_all, set_block_type, set_mark, split_block, toggle_mark,
    wrap_in, AttrSpec, AttrValue, Attrs, Command, Dispatch, EditorState, KeyPress, Keymap, Mark,
    MarkSpec, MarkType, Node, NodeSpec, NodeType, Plugin, PluginKey, PluginSet, ResolvedPos,
    Schema, SchemaBuilder, Selection, Slice, Transaction, Transform,
};
/// Re-export the DOM-bridge surface.
#[doc(no_inline)]
pub use taino_edit_dom::{Decoration, EditorView, ViewAction, ViewDesc, ViewPlugin};

/// A move-once container of DOM-aware [`ViewPlugin`]s for the
/// [`TainoEditor`] `plugins` prop.
///
/// Dioxus props must be `Clone + PartialEq`; a bare
/// `Vec<Box<dyn ViewPlugin>>` is neither, so the plugins live behind a shared
/// cell that is cheap to clone. They are installed on the view exactly once
/// at mount and never compared afterwards, so this type is deliberately
/// "always equal": changing the prop after mount has no effect and must not
/// trigger a re-render.
///
/// ```ignore
/// use taino_edit_dioxus::{TainoEditor, ViewPlugins};
/// use taino_edit_table_view::TableView;
///
/// rsx! { TainoEditor { state, plugins: ViewPlugins::new(vec![Box::new(TableView::new())]) } }
/// ```
#[derive(Clone, Default)]
pub struct ViewPlugins(PluginCell);

/// The shared, take-once backing store behind [`ViewPlugins`].
type PluginCell = Rc<RefCell<Option<Vec<Box<dyn ViewPlugin>>>>>;

impl ViewPlugins {
    /// Wrap a set of view plugins for the `plugins` prop.
    pub fn new(plugins: Vec<Box<dyn ViewPlugin>>) -> Self {
        Self(Rc::new(RefCell::new(Some(plugins))))
    }

    /// Take the plugins out, leaving the container empty. Called once at
    /// mount; any later call yields an empty vec.
    fn take(&self) -> Vec<Box<dyn ViewPlugin>> {
        self.0.borrow_mut().take().unwrap_or_default()
    }
}

impl PartialEq for ViewPlugins {
    // Mount-only and never re-read: every value compares equal so the prop
    // can't cause spurious re-renders of `TainoEditor`.
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for ViewPlugins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.0.borrow().as_ref().map_or(0, Vec::len);
        f.debug_struct("ViewPlugins").field("pending", &n).finish()
    }
}

/// A Dioxus component that renders an editor backed by a
/// [`Signal<EditorState>`]. Whenever the signal changes, the mounted DOM is
/// reconciled via [`EditorView::update`]; browser-side edits (typing, IME
/// commits, paste, selection changes) feed back into the signal by applying
/// the transforms the DOM bridge produces.
///
/// ```ignore
/// use dioxus::prelude::*;
/// use taino_edit_dioxus::{EditorState, TainoEditor};
///
/// #[component]
/// fn App(state: Signal<EditorState>) -> Element {
///     rsx! { TainoEditor { state } }
/// }
/// ```
#[component]
pub fn TainoEditor(
    state: Signal<EditorState>,
    /// Optional DOM-aware [`ViewPlugin`]s (e.g. `TableView` for table
    /// cell-drag-select + resize). Installed on the view at mount; the
    /// component wires pointer events to them and refreshes their
    /// decorations on every state change.
    #[props(default)]
    plugins: ViewPlugins,
) -> Element {
    // The mounted view + its event closures live here across renders.
    // EditorView is !Send + !Sync, which Dioxus signals tolerate.
    let mut runtime: Signal<Option<EditorRuntime>> = use_signal(|| None);

    // On every state change, patch the DOM and re-sync the selection.
    use_effect(move || {
        let snapshot = state.read().clone();
        if let Some(rt) = runtime.write().as_mut() {
            rt.view.update(snapshot.doc().clone());
            // Only re-sync the DOM selection when the editor is focused, so we
            // never steal focus back from another element (e.g. a search box).
            if rt.view.has_focus() && rt.view.read_selection() != Some(snapshot.selection()) {
                rt.applying_selection.set(true);
                let _ = rt.view.set_selection(snapshot.selection());
                rt.applying_selection.set(false);
            }
            // Refresh plugin decorations (e.g. table cell-selection
            // highlight) for the current selection.
            rt.view.refresh_view_decorations(Some(snapshot.selection()));
        }
    });

    let on_mounted = move |evt: Event<MountedData>| {
        let Some(element) = evt.data().downcast::<web_sys::Element>().cloned() else {
            return;
        };
        let snapshot = state.read().clone();
        let mut view = EditorView::mount(
            snapshot.doc().clone(),
            snapshot.schema().clone(),
            element.clone(),
        );
        view.set_view_plugins(plugins.take());
        view.refresh_view_decorations(Some(snapshot.selection()));
        let applying = Rc::new(Cell::new(false));
        let closures = wire_events(&element, runtime, state, applying.clone());
        runtime.set(Some(EditorRuntime {
            view,
            closures,
            applying_selection: applying,
        }));
    };

    rsx! {
        div {
            class: "taino-editor",
            onmounted: on_mounted,
        }
    }
}

/// What a mounted `TainoEditor` owns. Dropping this both drops the view
/// (frees the DOM-bound `EditorView`) and detaches every event listener.
struct EditorRuntime {
    view: EditorView,
    #[allow(dead_code)] // kept alive so the listeners they back stay attached.
    closures: Vec<EventCloser>,
    /// Set while the effect pushes state's selection into the DOM, so the
    /// `selectionchange` listener can ignore the resulting echo.
    applying_selection: Rc<Cell<bool>>,
}

/// A `Closure` registered on a DOM target; on drop the listener is removed.
struct EventCloser {
    event: &'static str,
    target: web_sys::EventTarget,
    closure: Closure<dyn FnMut(web_sys::Event)>,
    /// Whether the listener was registered in the capture phase (must match on
    /// removal). Used for `scroll`, which does not bubble.
    capture: bool,
}

impl Drop for EventCloser {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback_and_bool(
            self.event,
            self.closure.as_ref().unchecked_ref(),
            self.capture,
        );
    }
}

fn push_listener(
    closers: &mut Vec<EventCloser>,
    target: web_sys::EventTarget,
    event: &'static str,
    closure: Closure<dyn FnMut(web_sys::Event)>,
) {
    push_listener_capture(closers, target, event, closure, false);
}

fn push_listener_capture(
    closers: &mut Vec<EventCloser>,
    target: web_sys::EventTarget,
    event: &'static str,
    closure: Closure<dyn FnMut(web_sys::Event)>,
    capture: bool,
) {
    if target
        .add_event_listener_with_callback_and_bool(event, closure.as_ref().unchecked_ref(), capture)
        .is_ok()
    {
        closers.push(EventCloser {
            event,
            target,
            closure,
            capture,
        });
    }
}

fn wire_events(
    el: &web_sys::Element,
    runtime: Signal<Option<EditorRuntime>>,
    state: Signal<EditorState>,
    applying_selection: Rc<Cell<bool>>,
) -> Vec<EventCloser> {
    let target: web_sys::EventTarget = el.clone().into();
    let mut closers: Vec<EventCloser> = Vec::new();

    // `input`: text typed or deleted in a text node.
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        if let Some(Some(t)) = with_view(runtime, |v| v.read_dom_changes()) {
            apply_transform(state, &t);
        }
    });
    push_listener(&mut closers, target.clone(), "input", cb);

    // IME composition: suspend reads while composing, commit on end.
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        with_view(runtime, |v| v.composition_start());
    });
    push_listener(&mut closers, target.clone(), "compositionstart", cb);

    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        let t = with_view(runtime, |v| {
            v.composition_end();
            v.read_dom_changes()
        })
        .flatten();
        if let Some(t) = t {
            apply_transform(state, &t);
        }
    });
    push_listener(&mut closers, target.clone(), "compositionend", cb);

    // Paste: prefer Markdown, then HTML, then plain text — all sanitised
    // through the schema-aware paths in core.
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
        let t = with_view(runtime, |v| {
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
        if let Some(t) = t {
            apply_transform(state, &t);
        }
    });
    push_listener(&mut closers, target.clone(), "paste", cb);

    // Pointer events → view plugins (table cell-drag-select, resize). Each
    // fires `handle_view_event`; a returned action is applied to state.
    // No-op when no plugin claims the event.
    for kind in ["mousedown", "mousemove", "mouseup"] {
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
            if let Some(Some(action)) = with_view(runtime, |v| v.handle_view_event(&ev)) {
                apply_view_action(state, action);
            }
        });
        push_listener(&mut closers, target.clone(), kind, cb);
    }

    // `selectionchange` only fires on `document`; mirror the browser
    // selection into state so toolbar/keymap commands see the right
    // anchor/head. Drop the echo from our own effect-driven set_selection.
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        let doc_target: web_sys::EventTarget = doc.into();
        let applying = applying_selection;
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
            if applying.get() {
                return;
            }
            let Some(Some(sel)) = with_view(runtime, |v| v.read_selection()) else {
                return;
            };
            let cur = state.peek().selection();
            if sel == cur {
                return;
            }
            let mut s = state;
            let next = {
                let snap = s.peek();
                let mut tx = snap.tr();
                tx.set_selection(sel);
                tx.no_history();
                snap.apply(tx)
            };
            s.set(next);
        });
        push_listener(&mut closers, doc_target, "selectionchange", cb);
    }

    // Reposition inline-decoration overlays when the layout shifts without a
    // document edit. `scroll` is captured (it doesn't bubble) so editor- or
    // ancestor-level scrolling is caught too; `resize` fires on `window`.
    if let Some(window) = web_sys::window() {
        let win_target: web_sys::EventTarget = window.unchecked_into();
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
            with_view(runtime, |v| v.reposition_inline_decorations());
        });
        push_listener_capture(&mut closers, win_target.clone(), "scroll", cb, true);
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
            with_view(runtime, |v| v.reposition_inline_decorations());
        });
        push_listener(&mut closers, win_target, "resize", cb);
    }

    closers
}

/// Run `f` against the mounted `EditorView`, if any.
fn with_view<R>(
    runtime: Signal<Option<EditorRuntime>>,
    f: impl FnOnce(&EditorView) -> R,
) -> Option<R> {
    runtime.peek().as_ref().map(|rt| f(&rt.view))
}

/// Apply a [`ViewAction`] produced by a view plugin to the state signal.
fn apply_view_action(mut state: Signal<EditorState>, action: ViewAction) {
    match action {
        ViewAction::Select(sel) => {
            let next = {
                let snap = state.peek();
                let mut tx = snap.tr();
                tx.set_selection(sel);
                tx.no_history();
                snap.apply(tx)
            };
            state.set(next);
        }
        ViewAction::Command(cmd) => {
            let snapshot = state.peek().clone();
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

/// Fold a DOM-bridge transform into the state signal.
fn apply_transform(mut state: Signal<EditorState>, tr: &Transform) {
    let next = {
        let snap = state.peek();
        let mut tx = snap.tr();
        let mut ok = true;
        for step in tr.steps() {
            if tx.transform().step(step.clone(), snap.schema()).is_err() {
                ok = false;
                break;
            }
        }
        if !ok {
            return;
        }
        snap.apply(tx)
    };
    state.set(next);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial plugin (all-default trait impl) for exercising `ViewPlugins`.
    struct Dummy;
    impl ViewPlugin for Dummy {}

    #[test]
    fn view_plugins_take_is_once() {
        let p = ViewPlugins::new(vec![Box::new(Dummy), Box::new(Dummy)]);
        assert_eq!(p.take().len(), 2, "first take yields the installed plugins");
        assert_eq!(
            p.take().len(),
            0,
            "the container is empty after the first take"
        );
    }

    #[test]
    fn view_plugins_always_compare_equal() {
        // Mount-only prop: every value is "equal" so it never forces a
        // re-render of `TainoEditor`.
        assert_eq!(
            ViewPlugins::new(vec![Box::new(Dummy)]),
            ViewPlugins::default()
        );
    }
}
