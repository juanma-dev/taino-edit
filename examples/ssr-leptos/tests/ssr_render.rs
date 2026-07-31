//! Server-side render of the editor page, on the host (no browser): the
//! response HTML must contain the initial document — the whole point of SSR
//! — serialized *exactly* as `doc_view_html` emits it, and must not mark the
//! static document editable (that only happens on hydration).

#![cfg(feature = "ssr")]

use leptos::prelude::*;
use ssr_leptos::{initial_state, Editor};
use taino_edit_leptos::doc_view_html;

#[test]
fn server_html_contains_the_exact_initial_document() {
    let owner = Owner::new();
    let html = owner.with(|| view! { <Editor /> }.to_html());

    let expected = doc_view_html(initial_state().doc());
    assert!(
        html.contains(&expected),
        "SSR output must embed the serialized initial document.\nexpected fragment:\n{expected}\ngot:\n{html}"
    );
    assert!(
        html.contains("Server-rendered with taino-edit"),
        "document text must be in the server HTML"
    );
    assert!(
        !html.contains("contenteditable"),
        "the server-rendered document must be read-only until hydration"
    );
}
