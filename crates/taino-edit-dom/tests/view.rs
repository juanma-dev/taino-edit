//! Phase 4 Unit A: `EditorView::mount` renders a document into a real
//! contenteditable DOM in headless Chromium.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use serde_json::json;
use taino_edit_core::{AttrSpec, DomSpec, MarkSpec, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_dom::{EditorView, ViewDesc};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, HtmlElement};

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
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        )
        .mark(
            "strong",
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

fn para(s: &Schema, t: &str) -> Node {
    let txt = s.text(t, vec![]).unwrap();
    s.node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap()
}

/// Mount the test inside a fresh detached `<div>` so tests don't leak DOM
/// state between each other.
fn mount(doc: Node, s: Schema) -> EditorView {
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    EditorView::mount(doc, s, root)
}

#[wasm_bindgen_test]
fn mounting_makes_the_root_contenteditable() {
    let s = schema();
    let doc = s
        .node("doc", Default::default(), vec![para(&s, "Hi")], vec![])
        .unwrap();
    let view = mount(doc, s);
    assert_eq!(
        view.root().get_attribute("contenteditable").as_deref(),
        Some("true")
    );
}

#[wasm_bindgen_test]
fn renders_paragraph_text_into_dom() {
    let s = schema();
    let doc = s
        .node("doc", Default::default(), vec![para(&s, "Hello")], vec![])
        .unwrap();
    let view = mount(doc, s);
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    assert_eq!(html.inner_html(), "<p>Hello</p>");
    assert_eq!(view.children().len(), 1);
}

#[wasm_bindgen_test]
fn wraps_text_with_each_mark() {
    let s = schema();
    let strong = s.mark_type("strong").unwrap().create(Default::default());
    let em = s.mark_type("em").unwrap().create(Default::default());
    let plain = s.text("Hello ", vec![]).unwrap();
    let bold = s.text("world", vec![strong]).unwrap();
    let bold_em = s.text("!", em.add_to_set(bold.marks())).unwrap();
    let p = s
        .node(
            "paragraph",
            Default::default(),
            vec![plain, bold, bold_em],
            vec![],
        )
        .unwrap();
    let doc = s.node("doc", Default::default(), vec![p], vec![]).unwrap();

    let view = mount(doc, s);
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    let inner = html.inner_html();
    assert!(inner.contains("Hello "), "plain text is rendered: {inner}");
    assert!(
        inner.contains("<strong>world</strong>"),
        "strong wraps `world`: {inner}"
    );
    // The "!" run carries both strong and em (sorted by id), so it gets two
    // nested wrappers.
    assert!(
        inner.contains("<em>") && inner.contains("</em>"),
        "em present: {inner}"
    );
}

#[wasm_bindgen_test]
fn heading_uses_level_attr_to_pick_tag() {
    let s = schema();
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("level".into(), json!(2));
    let h = s
        .node(
            "heading",
            attrs,
            vec![s.text("Title", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    let doc = s.node("doc", Default::default(), vec![h], vec![]).unwrap();

    let view = mount(doc, s);
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    assert_eq!(html.inner_html(), "<h2>Title</h2>");
}

#[wasm_bindgen_test]
fn view_desc_tree_mirrors_the_document() {
    let s = schema();
    let doc = s
        .node(
            "doc",
            Default::default(),
            vec![para(&s, "a"), para(&s, "b")],
            vec![],
        )
        .unwrap();
    let view = mount(doc, s);

    assert_eq!(view.children().len(), 2);
    for (i, expected) in ["a", "b"].iter().enumerate() {
        match &view.children()[i] {
            ViewDesc::Element {
                node,
                dom,
                children,
            } => {
                assert_eq!(node.node_type().name(), "paragraph");
                assert_eq!(dom.tag_name().to_lowercase(), "p");
                assert_eq!(children.len(), 1);
                match &children[0] {
                    ViewDesc::Text { node, .. } => {
                        assert_eq!(node.text(), Some(*expected))
                    }
                    other => panic!("expected text desc, got {other:?}"),
                }
            }
            other => panic!("expected element desc, got {other:?}"),
        }
    }
}

#[wasm_bindgen_test]
fn re_mounting_clears_previous_dom() {
    let s = schema();
    let document = web_sys::window().unwrap().document().unwrap();
    let root: Element = document.create_element("div").unwrap();
    // Pre-existing junk in the root.
    let junk = document.create_element("span").unwrap();
    junk.set_text_content(Some("OLD"));
    let _ = root.append_child(&junk);

    let doc = s
        .node("doc", Default::default(), vec![para(&s, "NEW")], vec![])
        .unwrap();
    let view = EditorView::mount(doc, s, root);
    let html: HtmlElement = view.root().clone().dyn_into().unwrap();
    assert_eq!(html.inner_html(), "<p>NEW</p>");
}
