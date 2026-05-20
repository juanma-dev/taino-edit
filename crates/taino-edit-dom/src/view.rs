//! [`EditorView`] — mount a document into a `contenteditable` element and
//! own the [`ViewDesc`] tree that mirrors it.
//!
//! v0.1 / Unit A: initial render only. Incremental diff/patch, selection
//! sync, `MutationObserver`, IME and clipboard land in subsequent units of
//! Phase 4.

use taino_edit_core::{DomSpec, Node, Schema};
use web_sys::{Document, Element};

use crate::desc::ViewDesc;

/// The DOM-bound editor view.
#[derive(Debug)]
pub struct EditorView {
    root: Element,
    schema: Schema,
    doc: Node,
    /// Descriptors mirroring `doc.content()` children. The document node
    /// itself is "transparent" — its children become the direct children of
    /// the root element.
    children: Vec<ViewDesc>,
}

impl EditorView {
    /// Mount `doc` into `root`, marking the latter `contenteditable` and
    /// replacing any pre-existing children.
    pub fn mount(doc: Node, schema: Schema, root: Element) -> Self {
        let _ = root.set_attribute("contenteditable", "true");
        let document = root
            .owner_document()
            .expect("root element has an owner Document");

        // Empty `root`.
        while let Some(child) = root.first_child() {
            let _ = root.remove_child(&child);
        }

        let mut children = Vec::with_capacity(doc.child_count());
        for child in doc.content().iter() {
            let desc = render(child, &document);
            let _ = root.append_child(&desc.dom_node());
            children.push(desc);
        }

        EditorView {
            root,
            schema,
            doc,
            children,
        }
    }

    /// The mounted root element.
    pub fn root(&self) -> &Element {
        &self.root
    }

    /// The schema this view was mounted against.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The current document.
    pub fn doc(&self) -> &Node {
        &self.doc
    }

    /// The view descriptors mirroring the document's top-level children.
    pub fn children(&self) -> &[ViewDesc] {
        &self.children
    }
}

/// Build a `ViewDesc` for `node`, creating its DOM subtree along the way.
fn render(node: &Node, document: &Document) -> ViewDesc {
    if node.is_text() {
        return render_text(node, document);
    }

    let dom_el = match node.node_type().spec().to_dom {
        Some(f) => create_element(document, &f(node)),
        // Transparent / unrendered nodes still need *some* container so
        // editing inside them works; a `<span>` is the conservative default.
        None => document
            .create_element("span")
            .expect("create_element succeeds for `span`"),
    };

    let mut children = Vec::with_capacity(node.child_count());
    for child in node.content().iter() {
        let cd = render(child, document);
        let _ = dom_el.append_child(&cd.dom_node());
        children.push(cd);
    }

    ViewDesc::Element {
        node: node.clone(),
        dom: dom_el,
        children,
    }
}

/// Render a text node, wrapping it with the DOM elements declared by its
/// marks (innermost = the raw text node).
fn render_text(node: &Node, document: &Document) -> ViewDesc {
    let text_node = document.create_text_node(node.text().unwrap_or(""));
    let mut current: web_sys::Node = text_node.clone().into();
    let mut wrapper: Option<Element> = None;
    for mark in node.marks() {
        let Some(f) = mark.mark_type().spec().to_dom else {
            continue;
        };
        let el = create_element(document, &f(mark));
        let _ = el.append_child(&current);
        current = el.clone().into();
        wrapper = Some(el);
    }
    ViewDesc::Text {
        node: node.clone(),
        text: text_node,
        wrapper,
    }
}

/// Materialize a [`DomSpec`] into a `web_sys::Element` (tag + attrs).
fn create_element(document: &Document, spec: &DomSpec) -> Element {
    let el = document
        .create_element(spec.tag())
        .expect("create_element succeeds for spec tag");
    for (name, value) in spec.attrs() {
        let _ = el.set_attribute(name, value);
    }
    el
}
