//! [`EditorView`] — mount a document into a `contenteditable` element and
//! own the [`ViewDesc`] tree that mirrors it.
//!
//! v0.1 / Unit A: initial render only. Incremental diff/patch, selection
//! sync, `MutationObserver`, IME and clipboard land in subsequent units of
//! Phase 4.

use std::cell::Cell;

use taino_edit_core::{DomSpec, Fragment, Node, Schema, Selection, Slice, Transform};
use wasm_bindgen::JsValue;
use web_sys::{Document, Element};

use crate::decoration::Decoration;
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
    /// `true` while an IME composition is in progress — adapters wire
    /// `compositionstart`/`compositionend` to flip it. While set,
    /// [`read_dom_changes`](EditorView::read_dom_changes) returns `None` so
    /// transient intermediate-glyph states never trigger transactions.
    composing: Cell<bool>,
    /// Decorations currently applied on top of the rendered DOM.
    decorations: Vec<Decoration>,
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
            composing: Cell::new(false),
            decorations: Vec::new(),
        }
    }

    /// Replace the set of decorations applied on top of the rendered DOM.
    /// Previous decorations are removed; new ones are applied. Decorations
    /// that target positions outside the current document are silently
    /// skipped.
    pub fn set_decorations(&mut self, decorations: Vec<Decoration>) {
        for d in self.decorations.clone() {
            apply_decoration(&self.children, &d, false);
        }
        for d in &decorations {
            apply_decoration(&self.children, d, true);
        }
        self.decorations = decorations;
    }

    /// The decorations currently applied.
    pub fn decorations(&self) -> &[Decoration] {
        &self.decorations
    }

    /// Wire this from the host's `compositionstart` event handler.
    pub fn composition_start(&self) {
        self.composing.set(true);
    }

    /// Wire this from the host's `compositionend` event handler. The
    /// committed text is now stable in the DOM, so `read_dom_changes()`
    /// will once again report changes.
    pub fn composition_end(&self) {
        self.composing.set(false);
    }

    /// Whether an IME composition is in progress.
    pub fn is_composing(&self) -> bool {
        self.composing.get()
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

    /// Detect a divergence between a text node's DOM contents and its
    /// document text (the typical effect of typing/IME), and produce a
    /// [`Transform`] that, when applied to the current doc, brings them back
    /// into sync. Returns `None` if every text run matches.
    ///
    /// v0.1 reports the first divergent text run. Adapters wire this up
    /// behind a `MutationObserver` so it runs on every browser-side edit.
    /// During an IME composition (see [`composition_start`]) it returns
    /// `None` so transient glyph states never produce transactions; the
    /// host commits the change from the `compositionend` handler after
    /// calling [`composition_end`].
    ///
    /// [`composition_start`]: EditorView::composition_start
    /// [`composition_end`]: EditorView::composition_end
    pub fn read_dom_changes(&self) -> Option<Transform> {
        if self.composing.get() {
            return None;
        }
        let mut found = None;
        collect_text_changes(&self.children, 0, &mut |desc, doc_pos| {
            if found.is_some() {
                return;
            }
            if let ViewDesc::Text { node, text, .. } = desc {
                let dom_data = text.data();
                let doc_text = node.text().unwrap_or("");
                if dom_data != doc_text {
                    found = Some((doc_pos, doc_text.chars().count(), dom_data, node.clone()));
                }
            }
        });
        let (pos, old_len, new_text, prev_text_node) = found?;
        let mut transform = Transform::new(self.doc.clone());
        let replacement = if new_text.is_empty() {
            Slice::empty()
        } else {
            let new_node = self
                .schema
                .text(&new_text, prev_text_node.marks().to_vec())
                .ok()?;
            Slice::new(Fragment::from_node(new_node), 0, 0)
        };
        transform
            .replace(pos, pos + old_len, replacement, &self.schema)
            .ok()?;
        Some(transform)
    }

    /// The currently-selected document range (or, when the selection lies
    /// outside the mounted root, `None`).
    fn paste_range(&self) -> Option<(usize, usize)> {
        let sel = self.read_selection()?;
        Some(match sel {
            Selection::Text { anchor, head } => (anchor.min(head), anchor.max(head)),
            Selection::Node { pos } => {
                let len = self.doc.node_at(pos).map(|n| n.node_size()).unwrap_or(0);
                (pos, pos + len)
            }
            Selection::All => (0, self.doc.content().size()),
        })
    }

    /// Paste plain text at the current DOM selection, returning the
    /// resulting [`Transform`]. The text becomes a new text node with no
    /// marks; the prior selection is replaced (range or caret).
    pub fn paste_text(&self, text: &str) -> Option<Transform> {
        let (from, to) = self.paste_range()?;
        let mut transform = Transform::new(self.doc.clone());
        let slice = if text.is_empty() {
            Slice::empty()
        } else {
            let node = self.schema.text(text, vec![]).ok()?;
            Slice::new(Fragment::from_node(node), 0, 0)
        };
        transform.replace(from, to, slice, &self.schema).ok()?;
        Some(transform)
    }

    /// Paste HTML at the current DOM selection. The HTML is parsed through
    /// [`Schema::parse_html`] — which is already strict and depth-bounded,
    /// so untrusted clipboard content cannot inject schema-illegal
    /// structure — and the resulting blocks are spliced into the range.
    /// Returns `None` when parsing fails or the replacement would violate
    /// the schema for the destination.
    pub fn paste_html(&self, html: &str) -> Option<Transform> {
        let parsed = self.schema.parse_html(html).ok()?;
        let (from, to) = self.paste_range()?;
        let slice = Slice::new(parsed.content().clone(), 0, 0);
        let mut transform = Transform::new(self.doc.clone());
        transform.replace(from, to, slice, &self.schema).ok()?;
        Some(transform)
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

/// Walk descs in doc order, calling `visit` for each descriptor with its
/// absolute document position at the start of the descriptor's coverage.
fn collect_text_changes(
    descs: &[ViewDesc],
    base: usize,
    visit: &mut dyn FnMut(&ViewDesc, usize),
) -> usize {
    let mut pos = base;
    for d in descs {
        match d {
            ViewDesc::Text { node, .. } => {
                visit(d, pos);
                pos += node.node_size();
            }
            ViewDesc::Element { node, children, .. } => {
                collect_text_changes(children, pos + 1, visit);
                pos += node.node_size();
            }
        }
    }
    pos
}

// ---- decorations --------------------------------------------------------

fn apply_decoration(children: &[ViewDesc], deco: &Decoration, add: bool) {
    match deco {
        Decoration::Node { pos, class } => {
            if let Some(ViewDesc::Element { dom, .. }) = find_block_at(children, *pos) {
                let list = dom.class_list();
                if add {
                    let _ = list.add_1(class);
                } else {
                    let _ = list.remove_1(class);
                }
            }
        }
    }
}

fn find_block_at(children: &[ViewDesc], pos: usize) -> Option<&ViewDesc> {
    let mut cur = 0;
    for c in children {
        if pos == cur {
            return Some(c);
        }
        cur += c.node().node_size();
        if pos < cur {
            return None;
        }
    }
    None
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
