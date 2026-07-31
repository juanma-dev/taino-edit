//! taino-edit under Leptos **SSR + hydration**.
//!
//! The interesting part is what you *don't* see: `<TainoEditor>` is the same
//! component the CSR demo uses. Under `ssr` it server-renders the initial
//! document as plain HTML (curl this app — the text is in the response);
//! after the wasm bundle hydrates, the mount effect swaps in the live
//! editor over markup that is, by contract, structurally identical.

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, redo_command, undo_command, Bold, Heading, History,
    Italic, Paragraph,
};
use taino_edit_leptos::{
    set_block_type, toggle_mark, Attrs, Command, EditorState, NodeSpec, SchemaBuilder, TainoEditor,
    Transaction,
};

/// The document the server renders. Shared with the SSR test, which asserts
/// the serialized form of exactly this doc appears in the response HTML.
pub fn initial_state() -> EditorState {
    let base = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        );
    let exts: Vec<&dyn taino_edit_extensions::Extension> =
        vec![&Paragraph, &Heading, &Bold, &Italic, &History];
    let schema = build_schema_with(base, &exts, "doc").expect("schema builds");

    let strong = schema.mark_type("strong").unwrap().create(Attrs::new());
    let em = schema.mark_type("em").unwrap().create(Attrs::new());

    let title = schema
        .node(
            "heading",
            Attrs::from_iter([("level".into(), serde_json::json!(1u64))]),
            vec![schema
                .text("Server-rendered with taino-edit", vec![])
                .unwrap()],
            vec![],
        )
        .unwrap();
    let p1 = schema
        .node(
            "paragraph",
            Default::default(),
            vec![
                schema
                    .text("This document arrived as HTML: ", vec![])
                    .unwrap(),
                schema
                    .text("view source or curl this page", vec![strong])
                    .unwrap(),
                schema
                    .text(" and it is already there, before any wasm loads. ", vec![])
                    .unwrap(),
                schema
                    .text("Once hydrated, it becomes a live editor.", vec![em])
                    .unwrap(),
            ],
            vec![],
        )
        .unwrap();
    let p2 = schema
        .node(
            "paragraph",
            Default::default(),
            vec![schema
                .text("Try it: bold, italics, headings, undo/redo.", vec![])
                .unwrap()],
            vec![],
        )
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![title, p1, p2], vec![])
        .unwrap();
    EditorState::new(doc, schema)
}

/// The editor page: toolbar + `<TainoEditor>` + a keymap, initialized with
/// [`initial_state`].
#[component]
pub fn Editor() -> impl IntoView {
    let state = RwSignal::new(initial_state());
    let schema = state.with_untracked(|s| s.schema().clone());

    let exts: Vec<&dyn taino_edit_extensions::Extension> =
        vec![&Paragraph, &Heading, &Bold, &Italic, &History];
    let keymap = build_keymap_with(&exts, &schema, /*mac=*/ false);

    // Apply a command, fold the result back into the state signal.
    let run_command = move |cmd: &Command| {
        let mut next = None;
        let snapshot = state.get_untracked();
        {
            let mut d = |tx: Transaction| next = Some(snapshot.apply(tx));
            cmd(&snapshot, Some(&mut d));
        }
        if let Some(n) = next {
            state.set(n);
        }
    };
    // Commands are `!Send`; park each in a local slot so `Copy` handles can
    // reach them from the (Send) click closures.
    let slot = |cmd: Command| -> StoredValue<Command, LocalStorage> { StoredValue::new_local(cmd) };
    let run_slot = move |s: StoredValue<Command, LocalStorage>| {
        s.with_value(|cmd| run_command(cmd));
    };

    let strong = schema.mark_type("strong").unwrap().clone();
    let em = schema.mark_type("em").unwrap().clone();
    let bold_slot = slot(toggle_mark(strong));
    let italic_slot = slot(toggle_mark(em));
    let h1_slot = slot(set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(1u64))]),
    ));
    let h2_slot = slot(set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(2u64))]),
    ));
    let para_slot = slot(set_block_type("paragraph", Attrs::new()));
    let undo_slot = slot(undo_command());
    let redo_slot = slot(redo_command());

    // Toolbar buttons must not steal focus from the editor.
    let keep_focus = move |ev: leptos::ev::MouseEvent| ev.prevent_default();

    view! {
        <main style="font-family: system-ui; max-width: 46rem; margin: 1.5rem auto; padding: 0 1rem;">
            <header>
                <h1>"taino-edit — SSR demo"</h1>
                <p style="color:#555;">
                    "The editor below was in the server's HTML response. Disable JavaScript "
                    "and reload: the document still renders (read-only). With JavaScript, the "
                    "wasm bundle hydrates it into a fully live editor."
                </p>
            </header>
            <div role="toolbar" style="display:flex; flex-wrap:wrap; gap:.4rem; margin-bottom:.5rem;">
                <button on:mousedown=keep_focus on:click=move |_| run_slot(bold_slot)>"Bold"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(italic_slot)>"Italic"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(h1_slot)>"H1"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(h2_slot)>"H2"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(para_slot)>"¶"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(undo_slot)>"Undo"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(redo_slot)>"Redo"</button>
            </div>
            <style>
                ".taino-editor { border: 1px solid #ccc; border-radius: 6px; padding: .75rem 1rem; min-height: 10rem; }
                 .taino-editor:focus { outline: 2px solid #4a90d9; }"
            </style>
            <TainoEditor state=state keymap=keymap />
        </main>
    }
}

/// Router shell around [`Editor`] (a single route; the router is what lets
/// `leptos_axum::generate_route_list` drive the server).
#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Routes fallback=|| "Not found.">
                <Route path=path!("") view=Editor />
            </Routes>
        </Router>
    }
}

/// The HTML shell the server wraps around [`App`].
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>"taino-edit — Leptos SSR demo"</title>
                <AutoReload options=options.clone() />
                <HydrationScripts options />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

/// Client entry point: hydrate the server-rendered body.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
