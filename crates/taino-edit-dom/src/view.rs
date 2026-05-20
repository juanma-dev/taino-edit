//! [`EditorView`] — mount a document into a `contenteditable` element and
//! own the [`ViewDesc`] tree that mirrors it.
//!
//! v0.1 / Unit A: initial render only. Incremental diff/patch, selection
//! sync, `MutationObserver`, IME and clipboard land in subsequent units of
//! Phase 4.

use taino_edit_core::{DomSpec, Node, Schema, Selection};
use wasm_bindgen::JsValue;
use web_sys::{Document, Element};

use crate::desc::ViewDesc;
use crate::position_map::{doc_pos_to_dom, dom_to_doc_pos};

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

    /// Write the editor selection to the browser's `window.getSelection()`.
    ///
    /// Text selections map both endpoints; node selections collapse to the
    /// node's start/end positions; an all-selection covers the whole root.
    /// Returns `Err` if the underlying DOM call rejects (e.g. no window).
    pub fn set_selection(&self, sel: Selection) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let selection = window
            .get_selection()?
            .ok_or_else(|| JsValue::from_str("no Selection api"))?;

        let (anchor_pos, head_pos) = match sel {
            Selection::Text { anchor, head } => (anchor, head),
            Selection::Node { pos } => {
                let len = self.doc.node_at(pos).map(|n| n.node_size()).unwrap_or(0);
                (pos, pos + len)
            }
            Selection::All => (0, self.doc.content().size()),
        };

        let (anchor_node, anchor_off) = doc_pos_to_dom(&self.root, &self.children, anchor_pos)
            .ok_or_else(|| JsValue::from_str("anchor out of range"))?;
        let (focus_node, focus_off) = doc_pos_to_dom(&self.root, &self.children, head_pos)
            .ok_or_else(|| JsValue::from_str("head out of range"))?;

        selection.remove_all_ranges()?;
        selection.set_base_and_extent(&anchor_node, anchor_off, &focus_node, focus_off)
    }

    /// Read the current browser selection and translate its endpoints back
    /// into a doc-level [`Selection::Text`]. `None` if the browser has no
    /// selection (or anchor/focus are outside the mounted root).
    pub fn read_selection(&self) -> Option<Selection> {
        let window = web_sys::window()?;
        let selection = window.get_selection().ok().flatten()?;
        let anchor_node = selection.anchor_node()?;
        let focus_node = selection.focus_node()?;
        let anchor = dom_to_doc_pos(
            &self.root,
            &self.children,
            &anchor_node,
            selection.anchor_offset(),
        )?;
        let head = dom_to_doc_pos(
            &self.root,
            &self.children,
            &focus_node,
            selection.focus_offset(),
        )?;
        Some(Selection::Text { anchor, head })
    }

    /// Reconcile the mounted DOM with `new_doc`, performing minimal
    /// mutations: identical subtrees are kept, text-only changes set
    /// `nodeValue` in place, same-type elements recurse, and only nodes that
    /// truly changed are removed/replaced/appended.
    pub fn update(&mut self, new_doc: Node) {
        let document = self
            .root
            .owner_document()
            .expect("root element has an owner Document");
        let new_kids: Vec<Node> = new_doc.content().iter().cloned().collect();
        let new_descs = patch_children(&document, &self.root, &self.children, &new_kids);
        self.children = new_descs;
        self.doc = new_doc;
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

// ---- diff / patch -------------------------------------------------------

/// Same type + attrs + marks — i.e. only the inline content differs.
fn same_markup(a: &Node, b: &Node) -> bool {
    a.node_type() == b.node_type() && a.attrs() == b.attrs() && a.marks() == b.marks()
}

/// Patch the children of `parent_dom` in place. Returns the new descriptors.
fn patch_children(
    document: &Document,
    parent_dom: &Element,
    old: &[ViewDesc],
    new: &[Node],
) -> Vec<ViewDesc> {
    let mut result = Vec::with_capacity(new.len());
    for (i, new_node) in new.iter().enumerate() {
        if let Some(old_desc) = old.get(i) {
            if let Some(patched) = try_patch(document, old_desc, new_node) {
                result.push(patched);
                continue;
            }
            // Different enough that we must replace.
            let fresh = render(new_node, document);
            let _ = parent_dom.replace_child(&fresh.dom_node(), &old_desc.dom_node());
            result.push(fresh);
        } else {
            // New child past the old length: append.
            let fresh = render(new_node, document);
            let _ = parent_dom.append_child(&fresh.dom_node());
            result.push(fresh);
        }
    }
    // Remove leftover old DOM nodes the new tree no longer needs.
    for stale in old.iter().skip(new.len()) {
        let _ = parent_dom.remove_child(&stale.dom_node());
    }
    result
}

/// Try to update `old` in place to match `new`; return the new desc if the
/// patch could be applied, or `None` if the caller must remove + re-render.
fn try_patch(document: &Document, old: &ViewDesc, new: &Node) -> Option<ViewDesc> {
    // Structurally identical → keep the existing desc / DOM untouched.
    if old.node() == new {
        return Some(old.clone());
    }

    match old {
        ViewDesc::Text {
            node,
            text,
            wrapper,
        } => {
            if !new.is_text() || !same_markup(node, new) {
                return None;
            }
            text.set_data(new.text().unwrap_or(""));
            Some(ViewDesc::Text {
                node: new.clone(),
                text: text.clone(),
                wrapper: wrapper.clone(),
            })
        }
        ViewDesc::Element {
            node,
            dom,
            children,
        } => {
            if new.is_text() || node.node_type() != new.node_type() || node.attrs() != new.attrs() {
                return None;
            }
            let new_kids: Vec<Node> = new.content().iter().cloned().collect();
            let new_children = patch_children(document, dom, children, &new_kids);
            Some(ViewDesc::Element {
                node: new.clone(),
                dom: dom.clone(),
                children: new_children,
            })
        }
    }
}
