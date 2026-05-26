//! v0.5: inline (range-level) decorations are drawn as an **overlay** above
//! the text — a sibling of the editor root — so they highlight arbitrary
//! ranges (search hits, comment ranges, remote selections) without touching
//! the editable DOM. These browser tests assert the overlay geometry renders
//! and, crucially, that it does not disturb the typing read-back path.

#![cfg(target_arch = "wasm32")]

use taino_edit_core::{DomSpec, Node, NodeSpec, Schema, SchemaBuilder, Selection};
use taino_edit_dom::{Decoration, EditorView, ViewPlugin};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, Text};

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
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        )
        .top_node("doc")
        .build()
        .unwrap()
}

fn para(s: &Schema, t: &str) -> Node {
    s.node(
        "paragraph",
        Default::default(),
        vec![s.text(t, vec![]).unwrap()],
        vec![],
    )
    .unwrap()
}

fn doc(s: &Schema, ps: Vec<Node>) -> Node {
    s.node("doc", Default::default(), ps, vec![]).unwrap()
}

/// Mount `d` into a root that lives inside a host attached to `<body>` (so the
/// overlay — a sibling of the root — has a parent and real layout geometry).
/// Returns the view and the host (remove it to clean up).
fn mount_in_host(d: Node, s: Schema) -> (EditorView, Element) {
    let document = web_sys::window().unwrap().document().unwrap();
    let host = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&host).unwrap();
    let root = document.create_element("div").unwrap();
    host.append_child(&root).unwrap();
    let view = EditorView::mount(d, s, root);
    (view, host)
}

fn overlay(host: &Element) -> Option<Element> {
    host.query_selector(".taino-deco-layer").unwrap()
}

#[wasm_bindgen_test]
fn inline_decoration_draws_overlay_box_without_touching_editable_dom() {
    let s = schema();
    let (mut view, host) = mount_in_host(doc(&s, vec![para(&s, "Hello world")]), s);

    // "Hello" occupies doc positions 1..6 (pos 0 is before the paragraph).
    view.set_decorations(vec![Decoration::inline(1, 6, "hl")]);

    let layer = overlay(&host).expect("overlay layer created as a sibling of root");
    let boxes = layer.query_selector_all(".hl").unwrap();
    assert!(boxes.length() >= 1, "expected at least one highlight box");

    let first: Element = boxes.item(0).unwrap().dyn_into().unwrap();
    let rect = first.get_bounding_client_rect();
    assert!(
        rect.width() > 0.0 && rect.height() > 0.0,
        "highlight box should have real geometry: {}x{}",
        rect.width(),
        rect.height()
    );

    // The editable DOM is untouched: the paragraph still reads "Hello world"
    // and no decoration <span> was injected inside it.
    let root = view.root();
    let p: Element = root.first_child().unwrap().dyn_into().unwrap();
    assert_eq!(p.text_content().as_deref(), Some("Hello world"));
    assert!(
        p.query_selector("span").unwrap().is_none(),
        "inline decorations must not wrap text in the editable DOM"
    );

    let _ = host.parent_element().map(|b| b.remove_child(&host));
}

#[wasm_bindgen_test]
fn clearing_inline_decorations_empties_the_overlay() {
    let s = schema();
    let (mut view, host) = mount_in_host(doc(&s, vec![para(&s, "Hello world")]), s);

    view.set_decorations(vec![Decoration::inline(1, 6, "hl")]);
    assert!(overlay(&host).unwrap().child_element_count() >= 1);

    view.set_decorations(Vec::new());
    assert_eq!(
        overlay(&host).unwrap().child_element_count(),
        0,
        "clearing decorations must empty the overlay layer"
    );

    let _ = host.parent_element().map(|b| b.remove_child(&host));
}

#[wasm_bindgen_test]
fn multiline_range_draws_a_box_per_line() {
    // A narrow host forces the text to wrap, so a range across the wrap
    // produces more than one client rect → more than one box.
    let s = schema();
    let (mut view, host) = mount_in_host(
        doc(&s, vec![para(&s, "aaaaaaaaaa bbbbbbbbbb cccccccccc")]),
        s,
    );
    // Narrow the editor so the text wraps onto multiple visual lines.
    let _ = view.root().set_attribute("style", "width:40px");

    // Cover the whole (now wrapped) text run.
    view.set_decorations(vec![Decoration::inline(1, 32, "hl")]);

    let boxes = overlay(&host).unwrap().query_selector_all(".hl").unwrap();
    assert!(
        boxes.length() >= 2,
        "a wrapped range should draw one box per visual line, got {}",
        boxes.length()
    );

    let _ = host.parent_element().map(|b| b.remove_child(&host));
}

#[wasm_bindgen_test]
fn inline_overlay_does_not_disturb_text_readback() {
    // The whole point of the overlay design: a live inline decoration must not
    // break the typing read-back, which reads `text.data()` from the editable
    // text node.
    let s = schema();
    let (mut view, host) = mount_in_host(doc(&s, vec![para(&s, "Hello")]), s.clone());
    view.set_decorations(vec![Decoration::inline(1, 6, "hl")]);

    // Simulate typing `!` at the end of "Hello".
    let p = view.root().first_child().unwrap();
    let t: Text = p.first_child().unwrap().dyn_into().unwrap();
    t.set_data("Hello!");

    let transform = view
        .read_dom_changes()
        .expect("read-back still detects the edit with an overlay present");
    assert_eq!(transform.doc(), &doc(&s, vec![para(&s, "Hello!")]));

    let _ = host.parent_element().map(|b| b.remove_child(&host));
}

/// A minimal third-party plugin: highlight one fixed range. The real
/// search/comment use cases just compute the ranges dynamically.
struct Highlight {
    from: usize,
    to: usize,
}

impl ViewPlugin for Highlight {
    fn decorations(&self, _view: &EditorView, _sel: Option<Selection>) -> Vec<Decoration> {
        vec![Decoration::inline(self.from, self.to, "hl")]
    }
}

#[wasm_bindgen_test]
fn inline_decorations_flow_through_a_view_plugin() {
    let s = schema();
    let (mut view, host) = mount_in_host(doc(&s, vec![para(&s, "Hello world")]), s);

    // The supported third-party path: a plugin contributes inline decorations
    // and the adapter calls `refresh_view_decorations` on every state change.
    view.set_view_plugins(vec![Box::new(Highlight { from: 1, to: 6 })]);
    view.refresh_view_decorations(None);

    let boxes = overlay(&host).unwrap().query_selector_all(".hl").unwrap();
    assert!(
        boxes.length() >= 1,
        "a ViewPlugin must be able to contribute inline decorations"
    );

    let _ = host.parent_element().map(|b| b.remove_child(&host));
}

#[wasm_bindgen_test]
fn node_and_inline_decorations_coexist() {
    let s = schema();
    let (mut view, host) = mount_in_host(doc(&s, vec![para(&s, "Hello world")]), s);

    view.set_decorations(vec![
        Decoration::node(0, "block-hl"),
        Decoration::inline(1, 6, "hl"),
    ]);

    let root = view.root();
    let p: Element = root.first_child().unwrap().dyn_into().unwrap();
    assert!(
        p.class_list().contains("block-hl"),
        "node decoration applied"
    );
    assert!(
        overlay(&host)
            .unwrap()
            .query_selector_all(".hl")
            .unwrap()
            .length()
            >= 1,
        "inline decoration drawn alongside the node decoration"
    );

    let _ = host.parent_element().map(|b| b.remove_child(&host));
}
