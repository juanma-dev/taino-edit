//! `taino-edit-leptos` — the Leptos adapter for taino-edit.
//!
//! A thin reactive bridge: a [`TainoEditor`] component that mounts a
//! [`taino_edit_dom::EditorView`] inside its rendered `<div>` and reacts to
//! the editor state held in a `RwSignal<EditorState>`. The pure-Rust
//! `core`/`dom` layers do all the editing work; this crate is the glue that
//! makes them feel like a normal Leptos component.
//!
//! v0.1 / Unit A: component + state→view mount/update. Event wiring
//! (`input`, composition, `paste`, `keydown` ↔ keymap) lands in Unit B.

#![warn(missing_docs, rust_2018_idioms)]

use leptos::prelude::*;
use taino_edit_core::EditorState;
use taino_edit_dom::EditorView;
use wasm_bindgen::JsCast;

/// A Leptos component that renders an editor backed by a
/// `RwSignal<EditorState>`. Whenever the signal changes, the mounted DOM is
/// reconciled via [`EditorView::update`]; on unmount the underlying
/// [`EditorView`] is dropped.
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
    /// changes.
    state: RwSignal<EditorState>,
) -> impl IntoView {
    let node_ref = NodeRef::<leptos::html::Div>::new();
    // `EditorView` owns `web_sys::Element`s and is `!Send + !Sync`. Keep it in
    // Leptos's local-storage slot so the (Send+Sync) effect closures can
    // reach it through a Copy handle without capturing the value itself.
    let view_holder: StoredValue<Option<EditorView>, LocalStorage> = StoredValue::new_local(None);

    Effect::new(move |_| {
        let snapshot = state.get();
        let Some(div) = node_ref.get() else {
            return;
        };
        view_holder.update_value(|h| match h.as_mut() {
            Some(view) => view.update(snapshot.doc().clone()),
            None => {
                let element: web_sys::Element = div.unchecked_into();
                *h = Some(EditorView::mount(
                    snapshot.doc().clone(),
                    snapshot.schema().clone(),
                    element,
                ));
            }
        });
    });

    on_cleanup(move || {
        view_holder.set_value(None);
    });

    view! { <div node_ref=node_ref class="taino-editor"></div> }
}
