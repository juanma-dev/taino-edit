//! End-to-end demo: a `<TainoEditor>` mounted alongside a toolbar that
//! exercises every v0.1 extension (marks, headings, alignment, lists,
//! blockquote, code-block, link, image, case transforms, undo/redo), a
//! full `Mod-…` keymap wired on `keydown`, and a live JSON + HTML preview
//! of the doc. Build with `trunk serve` in this directory.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::ev::{KeyboardEvent, MouseEvent};
use leptos::prelude::*;
use taino_edit_extensions::{
    add_column_after, add_row_after, align_center, align_justify, align_left, align_right,
    build_keymap_with, build_schema_with, delete_column, delete_row, delete_table, insert_image,
    insert_table, lift_list_item, merge_cells, redo_command, remove_link, select_cell_range,
    set_column_width, set_link, split_cell, to_lowercase, to_uppercase, toggle_header_row,
    undo_command, wrap_in_bullet_list, wrap_in_ordered_list, Align, Blockquote, Bold, Code,
    CodeBlock, Heading, History, Image, Italic, Link, Lists, Paragraph, Table,
};
use taino_edit_leptos::{
    set_block_type, toggle_mark, wrap_in, Attrs, Command, Decoration, EditorState, EditorView,
    KeyPress, Keymap, Node, NodeSpec, SchemaBuilder, Selection, TainoEditor, Transaction,
    ViewPlugin,
};

/// A demo [`ViewPlugin`] that highlights every (case-insensitive) occurrence
/// of a search query with an inline decoration. The query is shared with the
/// search box; changing it + nudging the state signal re-runs `decorations`.
#[derive(Default)]
struct SearchHighlight {
    query: Rc<RefCell<String>>,
}

impl ViewPlugin for SearchHighlight {
    fn decorations(&self, view: &EditorView, _sel: Option<Selection>) -> Vec<Decoration> {
        let q: Vec<char> = self.query.borrow().chars().collect();
        if q.is_empty() {
            return Vec::new();
        }
        let mut ranges = Vec::new();
        collect_matches(view.doc(), 0, &q, &mut ranges);
        ranges
            .into_iter()
            .map(|(from, to)| Decoration::inline(from, to, "taino-search-hit"))
            .collect()
    }
}

/// Walk `parent`'s content in document order (mirroring the editor's position
/// model: a child element's content starts at `pos + 1`), pushing the doc
/// position range of every non-overlapping, ASCII-case-insensitive match of
/// `query` found in a text node.
fn collect_matches(parent: &Node, base: usize, query: &[char], out: &mut Vec<(usize, usize)>) {
    let mut pos = base;
    for child in parent.content().iter() {
        if child.is_text() {
            let chars: Vec<char> = child.text().unwrap_or("").chars().collect();
            let mut i = 0;
            while i + query.len() <= chars.len() {
                if (0..query.len()).all(|k| chars[i + k].eq_ignore_ascii_case(&query[k])) {
                    out.push((pos + i, pos + i + query.len()));
                    i += query.len();
                } else {
                    i += 1;
                }
            }
        } else {
            collect_matches(child, pos + 1, query, out);
        }
        pos += child.node_size();
    }
}

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
    let exts: Vec<&dyn taino_edit_extensions::Extension> = vec![
        &Paragraph,
        &Heading,
        &Bold,
        &Italic,
        &Code,
        &Link,
        &Image,
        &Align,
        &Blockquote,
        &CodeBlock,
        &Lists,
        &Table,
        &History,
    ];
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

    // ------- search highlight (inline decorations) ------------------------
    // The query cell is shared between the `SearchHighlight` plugin (moved
    // into the view) and the search box. `!Send`, so the box's handle lives
    // in a `LocalStorage` slot.
    let search_query = Rc::new(RefCell::new(String::new()));
    let search_for_plugin = search_query.clone();
    let search_holder: StoredValue<Rc<RefCell<String>>, LocalStorage> =
        StoredValue::new_local(search_query);
    let on_search = move |ev: leptos::ev::Event| {
        let value = event_target_value(&ev);
        search_holder.with_value(|q| *q.borrow_mut() = value);
        // Nudge the signal so `<TainoEditor>` re-runs its effect and refreshes
        // decorations — the query lives outside the document.
        state.update(|_| {});
    };

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

    // ------- toolbar helpers ----------------------------------------------
    let strong = schema.mark_type("strong").unwrap().clone();
    let em = schema.mark_type("em").unwrap().clone();

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

    // Static commands kept in their own StoredValue so each button-click
    // closure can grab a Copy handle (Box<dyn Fn> is !Send and is consumed
    // when called repeatedly without a slot).
    let slot = |cmd: Command| -> StoredValue<Command, LocalStorage> { StoredValue::new_local(cmd) };

    let code_mark = schema.mark_type("code").unwrap().clone();
    let bold_slot = slot(toggle_mark(strong));
    let italic_slot = slot(toggle_mark(em));
    let code_mark_slot = slot(toggle_mark(code_mark));
    let undo_slot = slot(undo_command());
    let redo_slot = slot(redo_command());
    let para_slot = slot(set_block_type("paragraph", Attrs::new()));
    let h1_slot = slot(set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(1u64))]),
    ));
    let h2_slot = slot(set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(2u64))]),
    ));
    let h3_slot = slot(set_block_type(
        "heading",
        Attrs::from_iter([("level".into(), serde_json::json!(3u64))]),
    ));
    let align_l_slot = slot(align_left());
    let align_c_slot = slot(align_center());
    let align_r_slot = slot(align_right());
    let align_j_slot = slot(align_justify());
    let upper_slot = slot(to_uppercase());
    let lower_slot = slot(to_lowercase());
    let bq_slot = slot(wrap_in("blockquote", Attrs::new()));
    let code_slot = slot(set_block_type("code_block", Attrs::new()));
    let ul_slot = slot(wrap_in_bullet_list());
    let ol_slot = slot(wrap_in_ordered_list());
    let lift_slot = slot(lift_list_item());
    let unlink_slot = slot(remove_link());

    // Table command slots (all caret-relative except insert).
    let table_slot = slot(insert_table(3, 3));
    let row_add_slot = slot(add_row_after());
    let col_add_slot = slot(add_column_after());
    let row_del_slot = slot(delete_row());
    let col_del_slot = slot(delete_column());
    let header_slot = slot(toggle_header_row());
    let split_slot = slot(split_cell());
    let table_del_slot = slot(delete_table());
    let wider_slot = slot(set_column_width(0, 220));
    let narrower_slot = slot(set_column_width(0, 80));

    let run_slot = move |s: StoredValue<Command, LocalStorage>| {
        s.with_value(|c| run_command(c));
    };

    // Merge the caret's whole row: select it end-to-end, then merge. The
    // demo inserts 3×3 tables, so selecting columns 0..=2 of row 0 covers a
    // full row; a real app drives `select_cell_range` from a mouse drag.
    let on_merge_row = move |_| {
        run_command(&select_cell_range((0, 0), (0, 2)));
        run_command(&merge_cells());
    };

    // Toolbar buttons must not steal focus from the editor (the contenteditable
    // selection collapses on focus loss, and `state.selection` would then
    // mirror an empty selection via `selectionchange`). `mousedown` runs
    // before focus moves, so preventDefault keeps the editor focused.
    let keep_focus = move |ev: MouseEvent| {
        ev.prevent_default();
    };

    let select_all = move |_| {
        let snapshot = state.get_untracked();
        let mut tx = snapshot.tr();
        tx.set_selection(Selection::All);
        state.set(snapshot.apply(tx));
    };

    // Link command: ask the user for a URL, then dispatch set_link.
    let on_link = move |_| {
        let url = web_sys::window().and_then(|w| w.prompt_with_message("URL:").ok().flatten());
        if let Some(href) = url.filter(|s| !s.is_empty()) {
            let cmd = set_link(href, None);
            run_command(&cmd);
        }
    };

    // Image command: ask for a URL + alt text, then dispatch insert_image.
    let on_image = move |_| {
        let win = web_sys::window();
        let src = win
            .as_ref()
            .and_then(|w| w.prompt_with_message("Image URL:").ok().flatten())
            .filter(|s| !s.is_empty());
        let Some(src) = src else { return };
        let alt = win
            .as_ref()
            .and_then(|w| w.prompt_with_message("Alt text:").ok().flatten());
        let cmd = insert_image(src, alt.filter(|s| !s.is_empty()));
        run_command(&cmd);
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

            <div role="toolbar" style="display:flex; flex-wrap:wrap; gap:.4rem; margin-bottom:.5rem; align-items:center;">
                <button on:mousedown=keep_focus on:click=move |_| run_slot(bold_slot)>"Bold"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(italic_slot)>"Italic"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(code_mark_slot)>"‹/› code"</button>
                <button on:mousedown=keep_focus on:click=on_link>"Link…"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(unlink_slot)>"Unlink"</button>
                <button on:mousedown=keep_focus on:click=on_image>"Image…"</button>
                <span style="width:.5rem"></span>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(para_slot)>"P"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(h1_slot)>"H1"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(h2_slot)>"H2"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(h3_slot)>"H3"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(bq_slot)>"❝ Quote"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(code_slot)>"<> Code"</button>
                <span style="width:.5rem"></span>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(align_l_slot)>"⇤"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(align_c_slot)>"≡"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(align_r_slot)>"⇥"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(align_j_slot)>"☰"</button>
                <span style="width:.5rem"></span>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(ul_slot)>"• List"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(ol_slot)>"1. List"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(lift_slot)>"⇤ Lift"</button>
                <span style="width:.5rem"></span>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(upper_slot)>"AA"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(lower_slot)>"aa"</button>
                <span style="width:.5rem"></span>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(undo_slot)>
                    {move || format!("Undo [{}]", undo_depth.get())}
                </button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(redo_slot)>
                    {move || format!("Redo [{}]", redo_depth.get())}
                </button>
                <button on:mousedown=keep_focus on:click=select_all>"Select all"</button>
            </div>

            <div role="toolbar" style="display:flex; flex-wrap:wrap; gap:.4rem; margin-bottom:.5rem; align-items:center;">
                <strong style="font-size:.8rem; color:#555;">"Table:"</strong>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(table_slot)>"⊞ Insert 3×3"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(row_add_slot)>"+ Row"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(col_add_slot)>"+ Col"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(row_del_slot)>"− Row"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(col_del_slot)>"− Col"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(header_slot)>"Header row"</button>
                <button on:mousedown=keep_focus on:click=on_merge_row>"Merge row"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(split_slot)>"Split cell"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(wider_slot)>"Col wider"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(narrower_slot)>"Col narrower"</button>
                <button on:mousedown=keep_focus on:click=move |_| run_slot(table_del_slot)>"Delete table"</button>
            </div>

            <div role="search" style="display:flex; gap:.4rem; margin-bottom:.5rem; align-items:center;">
                <strong style="font-size:.8rem; color:#555;">"Search:"</strong>
                <input
                    type="search"
                    placeholder="highlight matches…"
                    on:input=on_search
                    style="padding:.2rem .4rem; border:1px solid #ccc; border-radius:4px;"
                />
                <span style="font-size:.8rem; color:#888;">"(inline-decoration overlay)"</span>
            </div>

            <div on:keydown=on_keydown>
                <TainoEditor
                    state=state
                    plugins=vec![
                        Box::new(taino_edit_table_view::TableView::new()) as Box<dyn ViewPlugin>,
                        Box::new(SearchHighlight { query: search_for_plugin }) as Box<dyn ViewPlugin>,
                    ]
                />
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
                    <li>"Select a word, press " <code>"Mod-b"</code> ". Strong appears in both panels."</li>
                    <li>"Same with " <code>"Mod-i"</code> "."</li>
                    <li>"Place the caret in a paragraph, click " <strong>"H2"</strong> ". It becomes an h2."</li>
                    <li>"Click " <strong>"P"</strong> " (or press " <code>"Mod-Alt-0"</code> ") to turn it back."</li>
                    <li>"Click " <strong>"≡"</strong> " on a paragraph — its style emits " <code>"text-align: center"</code> "."</li>
                    <li>"Click " <strong>"• List"</strong> " to wrap the block in a bullet list. " <code>"Shift-Tab"</code> " lifts it back out (single-item case)."</li>
                    <li>"Click " <strong>"❝ Quote"</strong> " (" <code>"Mod->"</code> ") to wrap a paragraph in a blockquote."</li>
                    <li>"Click " <strong>"<> Code"</strong> " (" <code>"Mod-`"</code> ") to turn the block into a " <code>"<pre>"</code> "."</li>
                    <li>"Select a word, click " <strong>"AA"</strong> " — only the selection uppercases, marks preserved."</li>
                    <li>"Select a word, click " <strong>"Link…"</strong> ", paste a URL — " <code>"<a href>"</code> " wraps the text."</li>
                    <li>"Place the caret, click " <strong>"Image…"</strong> ", paste an image URL + alt text."</li>
                    <li>"Type some text. Press " <code>"Mod-z"</code> ". The undo depth shrinks; the doc rolls back. " <code>"Mod-Shift-z"</code> " redoes."</li>
                    <li>"Paste some text from another tab — only known tags survive (" <code>"<p>"</code> ", " <code>"<strong>"</code> ", " <code>"<em>"</code> ", " <code>"h1/h2/h3"</code> ", " <code>"<a>"</code> ", " <code>"<img>"</code> ", " <code>"<ul>/<ol>/<li>"</code> ", " <code>"<blockquote>"</code> ", " <code>"<pre>"</code> ")."</li>
                    <li>"Watch the live JSON / HTML panels stay in lockstep with the editor."</li>
                </ol>
            </details>
        </main>
    }
}
