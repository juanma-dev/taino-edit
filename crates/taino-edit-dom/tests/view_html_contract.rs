//! The `doc_view_html` ↔ `EditorView::mount` markup contract, checked in a
//! real browser: parsing the serializer's output must yield exactly the DOM
//! the view builds. This is what makes SSR/first-paint markup swap-for-swap
//! identical with the mounted editor (no visual change on hydration).
//!
//! Both sides are read back through `innerHTML` so the *browser* is the
//! normalizer: the mounted view is serialized by it, and the probe div
//! parses + re-serializes the `doc_view_html` string.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use serde_json::json;
use taino_edit_core::{AttrSpec, DomSpec, MarkSpec, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dom::{doc_view_html, EditorView};
use wasm_bindgen_test::*;
use web_sys::Element;

wasm_bindgen_test_configure!(run_in_browser);

fn schema() -> Schema {
    SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "paragraph",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                to_dom: Some(|_| DomSpec::element("p")),
                ..Default::default()
            },
        )
        .node(
            "heading",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                attrs: {
                    let mut m = HashMap::new();
                    m.insert(
                        "level".to_string(),
                        AttrSpec {
                            default: Some(json!(1)),
                        },
                    );
                    m
                },
                to_dom: Some(|n| {
                    let level = n.attrs().get("level").and_then(|v| v.as_u64()).unwrap_or(1);
                    DomSpec::element(&format!("h{level}"))
                }),
                ..Default::default()
            },
        )
        .node(
            "image",
            NodeSpec {
                group: Some("inline".into()),
                attrs: {
                    let mut m = HashMap::new();
                    m.insert("src".to_string(), AttrSpec { default: None });
                    m
                },
                to_dom: Some(|n| {
                    let src = n.attrs().get("src").and_then(|v| v.as_str()).unwrap_or("");
                    DomSpec::void("img").attr("src", src)
                }),
                ..Default::default()
            },
        )
        .node(
            "ghost",
            NodeSpec {
                content: Some("block+".into()),
                group: Some("block".into()),
                // No `to_dom`: exercises the transparent-node `<span>` case.
                ..Default::default()
            },
        )
        .node(
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        )
        .mark(
            "bold",
            MarkSpec {
                to_dom: Some(|_| DomSpec::element("strong")),
                ..Default::default()
            },
        )
        .mark(
            "em",
            MarkSpec {
                to_dom: Some(|_| DomSpec::element("em")),
                ..Default::default()
            },
        )
        .top_node("doc")
        .build()
        .unwrap()
}

fn host() -> Element {
    let document = web_sys::window().unwrap().document().unwrap();
    let host = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&host).unwrap();
    host
}

/// One document exercising every branch the serializer mirrors: heading
/// attrs, nested marks (order!), escaped text, a void atom with an
/// attribute, an *empty* textblock (trailing break), and a transparent node.
#[wasm_bindgen_test]
fn serializer_output_parses_to_the_mounted_dom() {
    let s = schema();
    let bold = s.mark_type("bold").unwrap().create(Default::default());
    let em = s.mark_type("em").unwrap().create(Default::default());

    let mut hattrs = std::collections::BTreeMap::new();
    hattrs.insert("level".into(), json!(2));
    let heading = s
        .node(
            "heading",
            hattrs,
            vec![s.text("Título <&>", vec![]).unwrap()],
            vec![],
        )
        .unwrap();

    let mut iattrs = std::collections::BTreeMap::new();
    iattrs.insert("src".into(), json!("pic \"1\".png"));
    let img = s.node("image", iattrs, vec![], vec![]).unwrap();
    let rich = s
        .node(
            "paragraph",
            Default::default(),
            vec![
                s.text("plain ", vec![]).unwrap(),
                s.text("both", vec![bold, em]).unwrap(),
                img,
            ],
            vec![],
        )
        .unwrap();

    let empty = s
        .node("paragraph", Default::default(), vec![], vec![])
        .unwrap();

    let inner = s
        .node(
            "paragraph",
            Default::default(),
            vec![s.text("in ghost", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    let ghost = s
        .node("ghost", Default::default(), vec![inner], vec![])
        .unwrap();

    let doc = s
        .node(
            "doc",
            Default::default(),
            vec![heading, rich, empty, ghost],
            vec![],
        )
        .unwrap();

    let root = host();
    let _view = EditorView::mount(doc.clone(), s, root.clone());

    let document = web_sys::window().unwrap().document().unwrap();
    let probe = document.create_element("div").unwrap();
    probe.set_inner_html(&doc_view_html(&doc));

    assert_eq!(
        probe.inner_html(),
        root.inner_html(),
        "doc_view_html must parse to exactly the DOM EditorView::mount builds"
    );
    // Belt-and-braces: the empty textblock kept its caret anchor through the
    // parse round-trip.
    assert!(root.inner_html().contains("data-taino-trailing-break"));
}
