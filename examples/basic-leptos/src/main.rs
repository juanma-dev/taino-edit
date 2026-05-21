//! End-to-end demo: a `<TainoEditor>` mounted alongside a toolbar, full
//! `Mod-…` keymap wired on `keydown`, and a live JSON + HTML preview of
//! the doc. Build with `trunk serve` in this directory.

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, redo_command, undo_command, Bold, Heading, History,
    Italic, Paragraph,
};
use taino_edit_leptos::{
    set_block_type, toggle_mark, Attrs, Command, EditorState, KeyPress, Keymap, NodeSpec,
    SchemaBuilder, Selection, TainoEditor, Transaction,
};

fn main() {
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    // ------- schema + initial doc -----------------------------------------
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

    let title = schema.text("Welcome to taino-edit", vec![]).unwrap();
    let h = schema
        .node(
            "heading",
            Attrs::from_iter([("level".into(), serde_json::json!(1u64))]),
            vec![title],
            vec![],
        )
        .unwrap();
    let body = schema
        .text(
            "Type, format, undo — every command goes through Rust.",
            vec![],
        )
        .unwrap();
    let p = schema
        .node("paragraph", Default::default(), vec![body], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![h, p], vec![])
        .unwrap();
    let state = RwSignal::new(EditorState::new(doc, schema.clone()));

    // ------- keymap (kept in local storage because it's !Send) ------------
    let keymap_holder: StoredValue<Keymap, LocalStorage> =
        StoredValue::new_local(build_keymap_with(&exts, &schema, /*mac=*/ false));

    let on_keydown = move |ev: KeyboardEvent| {
        let key = KeyPress {
            key: ev.key(),
            ctrl: ev.ctrl_key(),
            alt: ev.alt_key(),
            shift: ev.shift_key(),
            meta: ev.meta_key(),
        };
        let mut next = None;
        let mut dispatch = |tx: Transaction| {
            next = Some(state.get_untracked().apply(tx));
        };
        let handled = keymap_holder
            .with_value(|km| km.handle(&state.get_untracked(), &key, Some(&mut dispatch)));
        if let Some(n) = next {
            state.set(n);
        }
        if handled {
            ev.prevent_default();
        }
    };

    // ------- toolbar -------------------------------------------------------
    let strong = schema.mark_type("strong").unwrap().clone();
    let em = schema.mark_type("em").unwrap().clone();
    let bold_cmd = toggle_mark(strong);
    let italic_cmd = toggle_mark(em);

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

    // We can't move `bold_cmd` directly into the closure (it's a Box<dyn Fn>
    // and we want to use it more than once across button clicks). Stash each
    // command in its own StoredValue and run on demand.
    let bold_slot: StoredValue<Command, LocalStorage> = StoredValue::new_local(bold_cmd);
    let italic_slot: StoredValue<Command, LocalStorage> = StoredValue::new_local(italic_cmd);
    let undo_slot: StoredValue<Command, LocalStorage> = StoredValue::new_local(undo_command());
    let redo_slot: StoredValue<Command, LocalStorage> = StoredValue::new_local(redo_command());
    let para_slot: StoredValue<Command, LocalStorage> =
        StoredValue::new_local(set_block_type("paragraph", Attrs::new()));
    let h1_slot: StoredValue<Command, LocalStorage> = StoredValue::new_local(set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(1u64))]),
    ));
    let h2_slot: StoredValue<Command, LocalStorage> = StoredValue::new_local(set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(2u64))]),
    ));
    let h3_slot: StoredValue<Command, LocalStorage> = StoredValue::new_local(set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(3u64))]),
    ));

    let run_slot = move |slot: StoredValue<Command, LocalStorage>| {
        slot.with_value(|c| run_command(c));
    };

    let select_all = move |_| {
        let snapshot = state.get_untracked();
        let mut tx = snapshot.tr();
        tx.set_selection(Selection::All);
        state.set(snapshot.apply(tx));
    };

    // ------- live previews -------------------------------------------------
    let json_preview =
        Memo::new(move |_| serde_json::to_string_pretty(&state.get().doc().to_json()).unwrap());
    let html_preview = Memo::new(move |_| state.get().doc().to_html());
    let undo_depth = Memo::new(move |_| state.get().history().undo_depth());
    let redo_depth = Memo::new(move |_| state.get().history().redo_depth());

    view! {
        <main style="font-family: system-ui; max-width: 60rem; margin: 1.5rem auto; padding: 0 1rem;">
            <header>
                <h1>"taino-edit demo"</h1>
                <p style="color:#555;">
                    "Pure-Rust WYSIWYG editor running in Leptos. Every change you make below "
                    "goes through the same transforms and history machinery — see the JSON and "
                    "HTML panels update live."
                </p>
            </header>

            <div role="toolbar" style="display:flex; flex-wrap:wrap; gap:.4rem; margin-bottom:.5rem;">
                <button on:click=move |_| run_slot(bold_slot)>"Bold (Mod-b)"</button>
                <button on:click=move |_| run_slot(italic_slot)>"Italic (Mod-i)"</button>
                <span style="width:.5rem"></span>
                <button on:click=move |_| run_slot(para_slot)>"Paragraph (Mod-Alt-0)"</button>
                <button on:click=move |_| run_slot(h1_slot)>"H1 (Mod-Alt-1)"</button>
                <button on:click=move |_| run_slot(h2_slot)>"H2 (Mod-Alt-2)"</button>
                <button on:click=move |_| run_slot(h3_slot)>"H3 (Mod-Alt-3)"</button>
                <span style="width:.5rem"></span>
                <button on:click=move |_| run_slot(undo_slot)>
                    {move || format!("Undo (Mod-z) [{}]", undo_depth.get())}
                </button>
                <button on:click=move |_| run_slot(redo_slot)>
                    {move || format!("Redo (Mod-Shift-z) [{}]", redo_depth.get())}
                </button>
                <button on:click=select_all>"Select all (Mod-a)"</button>
            </div>

            <div on:keydown=on_keydown>
                <TainoEditor state=state />
            </div>

            <section style="margin-top:1.5rem; display:grid; grid-template-columns:1fr 1fr; gap:1rem;">
                <div>
                    <h2 style="font-size:1rem;">"Live JSON"</h2>
                    <pre
                        style="background:#f6f8fa; padding:.75rem; border-radius:4px;
                               overflow:auto; max-height:18rem; font-size:.85rem;">
                        {move || json_preview.get()}
                    </pre>
                </div>
                <div>
                    <h2 style="font-size:1rem;">"Live HTML"</h2>
                    <pre
                        style="background:#f6f8fa; padding:.75rem; border-radius:4px;
                               overflow:auto; max-height:18rem; font-size:.85rem;">
                        {move || html_preview.get()}
                    </pre>
                </div>
            </section>

            <details style="margin-top:1.5rem;">
                <summary>"Try this checklist"</summary>
                <ol>
                    <li>"Click in a word and press " <code>"Mod-b"</code> ". The strong mark should NOT apply (caret only)."</li>
                    <li>"Select a word, then " <code>"Mod-b"</code> ". Strong appears in both panels."</li>
                    <li>"Same with " <code>"Mod-i"</code> "."</li>
                    <li>"Place the caret in a paragraph and press " <code>"Mod-Alt-2"</code> ". It becomes an h2."</li>
                    <li><code>"Mod-Alt-0"</code> " turns it back into a paragraph."</li>
                    <li>"Type some text. Press " <code>"Mod-z"</code> ". The undo depth shrinks; the doc rolls back."</li>
                    <li>"Press " <code>"Mod-Shift-z"</code> " to redo."</li>
                    <li>"Paste some text from another tab — it lands as plain text, no markup leaks."</li>
                    <li>"Paste HTML (e.g. copy from a news article). Only known tags survive (<p>, <strong>, <em>, h1/h2/h3)."</li>
                    <li>"Watch the live JSON / HTML panels stay in lockstep with the editor."</li>
                </ol>
            </details>
        </main>
    }
}
