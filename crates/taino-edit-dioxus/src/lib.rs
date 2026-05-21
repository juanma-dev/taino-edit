//! `taino-edit-dioxus` — the Dioxus adapter for taino-edit.
//!
//! Mirrors `taino-edit-leptos`: a [`TainoEditor`] component takes a
//! [`Signal<EditorState>`] and mounts a [`taino_edit_dom::EditorView`]
//! inside its rendered `<div>`, reconciling the DOM on every signal
//! change.
//!
//! Status: Phase v0.2 minimum-viable adapter. The Leptos adapter is the
//! full-featured reference; this is the second-framework proof that the
//! `core` / `dom` layers are framework-agnostic in practice and not just
//! in theory. Full event-wiring parity (input → transform round-trip,
//! IME, paste, selectionchange) lands in v0.2.x — the platform pieces it
//! needs are already shipped.

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(missing_docs, rust_2018_idioms)]

use dioxus::prelude::*;

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
pub use taino_edit_dom::{Decoration, EditorView, ViewDesc};

/// A Dioxus component that renders an editor backed by a
/// [`Signal<EditorState>`]. Whenever the signal changes, the mounted DOM is
/// reconciled via [`EditorView::update`]. The view is created on first
/// mount inside the `onmounted` callback.
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
pub fn TainoEditor(state: Signal<EditorState>) -> Element {
    // Carry the mounted EditorView across renders. Signal-of-Option works
    // fine here because EditorView is !Send + !Sync — Dioxus signals don't
    // require those bounds.
    let mut runtime: Signal<Option<EditorView>> = use_signal(|| None);

    // On every state change, patch the DOM if the view is mounted.
    use_effect(move || {
        let snapshot = state.read().clone();
        if let Some(view) = runtime.write().as_mut() {
            view.update(snapshot.doc().clone());
            if view.read_selection() != Some(snapshot.selection()) {
                let _ = view.set_selection(snapshot.selection());
            }
        }
    });

    let on_mounted = move |evt: Event<MountedData>| {
        // Recover the underlying web-sys Element from the MountedData. On
        // the web target this downcast is the canonical pattern.
        let Some(html_el) = evt.data().downcast::<web_sys::Element>().cloned() else {
            return;
        };
        let snapshot = state.read().clone();
        let view = EditorView::mount(snapshot.doc().clone(), snapshot.schema().clone(), html_el);
        runtime.set(Some(view));
    };

    rsx! {
        div {
            class: "taino-editor",
            onmounted: on_mounted,
        }
    }
}
