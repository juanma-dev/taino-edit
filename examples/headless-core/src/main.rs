//! Server-side demo: build a schema, edit a document through the
//! standard `Transform` pipeline, and round-trip it through JSON and HTML
//! without ever touching `web-sys`. Run with `cargo run -p headless-core`.

use serde_json::json;
use taino_edit_core::{EditorState, Fragment, NodeSpec, SchemaBuilder, Selection, Slice};
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, Bold, Heading, History, Italic, Paragraph,
};

fn main() {
    // 1) Compose a schema from the v0.1 extension set, on top of the
    //    universal `doc` / `text` primitives.
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

    // 2) Build an initial document programmatically (`# Hi\n\nHello`).
    let title = schema.text("Hi", vec![]).unwrap();
    let heading = schema
        .node(
            "heading",
            std::iter::once(("level".to_string(), json!(1u64))).collect(),
            vec![title],
            vec![],
        )
        .unwrap();
    let body = schema.text("Hello", vec![]).unwrap();
    let para = schema
        .node("paragraph", Default::default(), vec![body], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![heading, para], vec![])
        .unwrap();
    let state = EditorState::new(doc, schema.clone());

    // 3) Edit through the standard Transform pipeline: append " world" to
    //    the paragraph by inserting at its end. Compute the position
    //    arithmetically: heading (Hi) takes 2 chars + 2 tokens = 4; before
    //    closing token of `paragraph` is 4 + 1 (open) + 5 ("Hello") = 10.
    let mut tx = state.tr();
    let inserted = schema.text(" world", vec![]).unwrap();
    let slice = Slice::new(Fragment::from_node(inserted), 0, 0);
    tx.transform()
        .insert(10, slice, &schema)
        .expect("schema-valid insertion");
    let state = state.apply(tx);
    println!("after edit  → {}", state.doc().text_content());

    // 4) Round-trip through JSON.
    let as_json = state.doc().to_json();
    let parsed = schema.node_from_json(&as_json).expect("re-parse");
    assert_eq!(state.doc(), &parsed);
    println!(
        "json bytes  → {}",
        serde_json::to_string(&as_json).unwrap().len()
    );

    // 5) Serialize to HTML (escaped) and parse it back (schema-validated).
    let html = state.doc().to_html();
    let from_html = schema.parse_html(&html).expect("html parses");
    assert_eq!(from_html.text_content(), state.doc().text_content());
    println!("html        → {html}");

    // 6) Drive a command (Bold) through the built keymap.
    let keymap = build_keymap_with(&exts, &schema, /*mac=*/ false);

    // Select the word "Hello" inside the paragraph: positions 5..10.
    let mut t = state.tr();
    t.set_selection(Selection::Text {
        anchor: 5,
        head: 10,
    });
    let state = state.apply(t);

    let mut next = None;
    {
        let mut dispatch = |tx: taino_edit_core::Transaction| {
            next = Some(state.apply(tx));
        };
        let handled = keymap.handle(
            &state,
            &taino_edit_core::KeyPress::key("b").ctrl(),
            Some(&mut dispatch),
        );
        assert!(handled, "Mod-b is bound by the Bold extension");
    }
    let bolded = next.expect("bold dispatched");
    println!("after bold  → {}", bolded.doc().to_html());

    // 7) Undo via the History extension. After Mod-z the strong mark is
    //    gone.
    let mut undone = None;
    {
        let mut dispatch = |tx| {
            undone = Some(bolded.apply(tx));
        };
        keymap.handle(
            &bolded,
            &taino_edit_core::KeyPress::key("z").ctrl(),
            Some(&mut dispatch),
        );
    }
    let undone = undone.expect("undo dispatched");
    println!("after undo  → {}", undone.doc().to_html());
}
