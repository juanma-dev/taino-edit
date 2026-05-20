//! `taino-edit-leptos` — the Leptos adapter for taino-edit.
//!
//! A thin reactive bridge: a [`TainoEditor`] component that mounts a
//! [`taino_edit_dom::EditorView`] inside its rendered `<div>` and reacts to
//! the editor state held in a `RwSignal<EditorState>`. The pure-Rust
//! `core`/`dom` layers do all the editing work; this crate is the glue that
//! makes them feel like a normal Leptos component.
//!
//! Browser events (`input`, `compositionstart`/`compositionend`, `paste`)
//! are wired automatically: each one runs the corresponding `EditorView`
//! method and folds the resulting transform into the state signal.

#![warn(missing_docs, rust_2018_idioms)]

use leptos::prelude::*;
use taino_edit_core::{EditorState, Transform};
use taino_edit_dom::EditorView;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

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
) -> impl IntoView {
    let node_ref = NodeRef::<leptos::html::Div>::new();
    // `EditorView` + its event closures are `!Send + !Sync`. Keep them in
    // Leptos's local-storage slot so the (Send+Sync) effect closures can
    // reach them through a Copy handle without capturing the value itself.
    let runtime: StoredValue<Option<EditorRuntime>, LocalStorage> = StoredValue::new_local(None);

    Effect::new(move |_| {
        let snapshot = state.get();
        let Some(div) = node_ref.get() else {
            return;
        };
        runtime.update_value(|rt| match rt.as_mut() {
            Some(r) => r.view.update(snapshot.doc().clone()),
            None => {
                let element: web_sys::Element = div.unchecked_into();
                let view = EditorView::mount(
                    snapshot.doc().clone(),
                    snapshot.schema().clone(),
                    element.clone(),
                );
                let closures = wire_events(&element, runtime, state);
                *rt = Some(EditorRuntime { view, closures });
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
) -> Vec<EventCloser> {
    let target: web_sys::EventTarget = el.clone().into();
    let mut closers = Vec::new();

    let mut register =
        |event: &'static str, closure: Closure<dyn FnMut(web_sys::Event)>| -> Option<()> {
            target
                .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
                .ok()?;
            closers.push(EventCloser {
                event,
                target: target.clone(),
                closure,
            });
            Some(())
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

    // Paste: prefer HTML, fall back to plain text; both go through the
    // schema's strict sanitiser in core.
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Ok(clip) = ev.dyn_into::<web_sys::ClipboardEvent>() else {
            return;
        };
        clip.prevent_default();
        let Some(data) = clip.clipboard_data() else {
            return;
        };
        let html = data.get_data("text/html").unwrap_or_default();
        let text = data.get_data("text/plain").unwrap_or_default();
        let transform = with_view(runtime, |v| {
            if !html.is_empty() {
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
