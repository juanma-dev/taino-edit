//! [`EditorView`] — mount a document into a `contenteditable` element and
//! own the [`ViewDesc`] tree that mirrors it.
//!
//! v0.1 / Unit A: initial render only. Incremental diff/patch, selection
//! sync, `MutationObserver`, IME and clipboard land in subsequent units of
//! Phase 4.

use std::cell::Cell;

use taino_edit_core::{Command, DomSpec, Fragment, Node, Schema, Selection, Slice, Transform};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element};

use crate::decoration::Decoration;
use crate::desc::ViewDesc;
use crate::position_map::{doc_pos_to_dom, dom_to_doc_pos};

/// What a [`ViewPlugin`] asks the editor to do in response to a DOM event.
pub enum ViewAction {
    /// Replace the selection (e.g. a cell drag selecting a range).
    Select(Selection),
    /// Run an editing [`Command`] against the state (e.g. a column resize
    /// reusing `set_column_width`). The adapter applies it to its state.
    Command(Command),
}

/// A DOM-aware editor plugin: it reacts to raw browser events and
/// contributes [`Decoration`]s, with access to the live [`EditorView`] (its
/// document, schema and DOM-position primitives). Extensions whose
/// behaviour is purely structural use the schema/keymap surface in
/// `taino-edit-extensions`; those needing real pointer interaction
/// (table cell-drag-select, column resizing, …) implement this instead.
///
/// Adapters wire the editor's pointer/keyboard events to
/// [`EditorView::handle_view_event`] and refresh decorations through
/// [`EditorView::refresh_view_decorations`]; a plugin therefore stays
/// framework-agnostic.
pub trait ViewPlugin {
    /// Handle a raw DOM event. Return an action to apply, or `None` to pass.
    fn handle_event(&self, _view: &EditorView, _event: &web_sys::Event) -> Option<ViewAction> {
        None
    }

    /// Decorations to render for the current document and selection.
    fn decorations(&self, _view: &EditorView, _selection: Option<Selection>) -> Vec<Decoration> {
        Vec::new()
    }
}

/// The DOM-bound editor view.
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
    /// DOM-aware plugins consulted for event handling and decorations.
    plugins: Vec<Box<dyn ViewPlugin>>,
    /// The overlay layer for inline (range-level) decorations, created lazily
    /// as a *sibling* of `root`. Kept out of `root` so it never shifts the
    /// root's child indexing that selection mapping depends on.
    overlay: Option<Element>,
}

impl std::fmt::Debug for EditorView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorView")
            .field("doc", &self.doc)
            .field("children", &self.children)
            .field("decorations", &self.decorations)
            .field("plugins", &self.plugins.len())
            .finish_non_exhaustive()
    }
}

impl Drop for EditorView {
    fn drop(&mut self) {
        // The inline-decoration overlay is a sibling of `root`, so it would
        // otherwise outlive the view when the adapter removes the editor.
        if let Some(layer) = &self.overlay {
            if let Some(parent) = layer.parent_element() {
                let _ = parent.remove_child(layer);
            }
        }
    }
}

impl EditorView {
    /// Mount `doc` into `root`, marking the latter `contenteditable` and
    /// replacing any pre-existing children. Also sets `tabindex="0"` so the
    /// editor is reachable via the keyboard's Tab focus chain (a11y baseline);
    /// callers can change it later with [`set_tabindex`](EditorView::set_tabindex).
    pub fn mount(doc: Node, schema: Schema, root: Element) -> Self {
        let _ = root.set_attribute("contenteditable", "true");
        if !root.has_attribute("tabindex") {
            let _ = root.set_attribute("tabindex", "0");
        }
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
            plugins: Vec::new(),
            overlay: None,
        }
    }

    /// Install the DOM-aware [`ViewPlugin`]s (replacing any existing set).
    pub fn set_view_plugins(&mut self, plugins: Vec<Box<dyn ViewPlugin>>) {
        self.plugins = plugins;
    }

    /// Offer a raw DOM event to each view plugin in turn; returns the first
    /// [`ViewAction`] a plugin produces (the adapter applies it to state).
    pub fn handle_view_event(&self, event: &web_sys::Event) -> Option<ViewAction> {
        for p in &self.plugins {
            if let Some(action) = p.handle_event(self, event) {
                return Some(action);
            }
        }
        None
    }

    /// Recompute decorations from every plugin for the given selection and
    /// apply them. Adapters call this after the state signal changes.
    pub fn refresh_view_decorations(&mut self, selection: Option<Selection>) {
        let decos: Vec<Decoration> = self
            .plugins
            .iter()
            .flat_map(|p| p.decorations(self, selection))
            .collect();
        self.set_decorations(decos);
    }

    /// Map a viewport point to the document position just before the
    /// innermost rendered node element under it (walking up from
    /// `elementFromPoint` to the nearest node in the view tree). Used by
    /// pointer-driven plugins (e.g. table cell drag-select). `None` if the
    /// point isn't over the editor.
    pub fn pos_at_point(&self, x: f32, y: f32) -> Option<usize> {
        let document = web_sys::window()?.document()?;
        let mut el = document.element_from_point(x, y)?;
        loop {
            if let Some(pos) = pos_before_element(&self.children, 0, &el) {
                return Some(pos);
            }
            let parent = el.parent_element()?;
            let same_root = parent.is_same_node(Some(self.root.as_ref()));
            if !same_root && !self.root.contains(Some(parent.as_ref())) {
                return None;
            }
            el = parent;
        }
    }

    /// The DOM element of the node that begins at document position `pos`
    /// (any depth — block or nested cell). Used by pointer plugins to read
    /// a cell's geometry (e.g. for column-resize hit-testing).
    pub fn node_dom_at(&self, pos: usize) -> Option<Element> {
        dom_element_at(&self.children, 0, pos).cloned()
    }

    /// Replace the set of decorations applied on top of the rendered DOM.
    /// Previous decorations are removed; new ones are applied. Decorations
    /// that target positions outside the current document are silently
    /// skipped.
    ///
    /// [`Node`](Decoration::Node) decorations toggle a CSS class on the target
    /// element; [`Inline`](Decoration::Inline) decorations are drawn as boxes
    /// in an overlay layer that is rebuilt wholesale on every call.
    pub fn set_decorations(&mut self, decorations: Vec<Decoration>) {
        // Node decorations: remove the previous class set, then add the new.
        for d in &self.decorations {
            if matches!(d, Decoration::Node { .. }) {
                apply_decoration(&self.children, d, false);
            }
        }
        for d in &decorations {
            if matches!(d, Decoration::Node { .. }) {
                apply_decoration(&self.children, d, true);
            }
        }
        // Inline decorations: redraw the overlay layer from scratch.
        self.render_inline_overlay(&decorations);
        self.decorations = decorations;
    }

    /// The decorations currently applied.
    pub fn decorations(&self) -> &[Decoration] {
        &self.decorations
    }

    /// Redraw the inline-decoration overlay from `decorations`. Each inline
    /// range becomes one box per client rect (so a range spanning lines draws
    /// several boxes), positioned over the text it covers. The overlay is a
    /// sibling of `root`, so it never alters the editable DOM — typing and the
    /// diff/patch read-back are unaffected.
    ///
    /// Boxes are recomputed on every state change (adapters call
    /// [`refresh_view_decorations`](Self::refresh_view_decorations) after each
    /// update); they are *not* re-positioned on scroll/resize alone.
    fn render_inline_overlay(&mut self, decorations: &[Decoration]) {
        let any_inline = decorations
            .iter()
            .any(|d| matches!(d, Decoration::Inline { .. }));

        // Don't create an overlay just to leave it empty.
        let Some(layer) = self.ensure_overlay(any_inline) else {
            return;
        };
        layer.set_inner_html("");
        if !any_inline {
            return;
        }
        let Some(document) = self.root.owner_document() else {
            return;
        };
        // The overlay sits at its containing block's origin; its own client
        // rect gives that origin in viewport coordinates, so each box can be
        // placed relative to it regardless of the positioning context.
        let origin = layer.get_bounding_client_rect();
        for d in decorations {
            let Decoration::Inline { from, to, class } = d else {
                continue;
            };
            let (Some((sn, so)), Some((en, eo))) = (
                doc_pos_to_dom(&self.root, &self.children, *from),
                doc_pos_to_dom(&self.root, &self.children, *to),
            ) else {
                continue;
            };
            let Ok(range) = document.create_range() else {
                continue;
            };
            if range.set_start(&sn, so).is_err() || range.set_end(&en, eo).is_err() {
                continue;
            }
            let Some(rects) = range.get_client_rects() else {
                continue;
            };
            for i in 0..rects.length() {
                let Some(r) = rects.get(i) else { continue };
                if r.width() <= 0.0 && r.height() <= 0.0 {
                    continue;
                }
                let Ok(box_el) = document.create_element("span") else {
                    continue;
                };
                let _ = box_el.set_attribute("class", class);
                let style = format!(
                    "position:absolute;left:{:.2}px;top:{:.2}px;width:{:.2}px;\
                     height:{:.2}px;pointer-events:none;",
                    r.left() - origin.left(),
                    r.top() - origin.top(),
                    r.width(),
                    r.height(),
                );
                let _ = box_el.set_attribute("style", &style);
                let _ = layer.append_child(&box_el);
            }
        }
    }

    /// The overlay layer, created as a sibling of `root` on first need. When
    /// `want` is false and no overlay exists yet, returns `None` (don't make
    /// one only to clear it). `None` too if `root` has no parent to host it.
    fn ensure_overlay(&mut self, want: bool) -> Option<Element> {
        if let Some(layer) = &self.overlay {
            return Some(layer.clone());
        }
        if !want {
            return None;
        }
        let parent = self.root.parent_element()?;
        let document = self.root.owner_document()?;
        let layer = document.create_element("div").ok()?;
        let _ = layer.set_attribute("class", "taino-deco-layer");
        let _ = layer.set_attribute(
            "style",
            "position:absolute;left:0;top:0;width:0;height:0;pointer-events:none;",
        );
        let _ = parent.append_child(&layer);
        self.overlay = Some(layer.clone());
        Some(layer)
    }

    /// Programmatically focus the editor.
    pub fn focus(&self) -> Result<(), JsValue> {
        let el: web_sys::HtmlElement = self.root.clone().dyn_into()?;
        el.focus()
    }

    /// Whether the editor is the document's active (focused) element.
    pub fn has_focus(&self) -> bool {
        let Some(document) = self.root.owner_document() else {
            return false;
        };
        let Some(active) = document.active_element() else {
            return false;
        };
        wasm_bindgen::JsValue::from(active) == wasm_bindgen::JsValue::from(&self.root)
    }

    /// Override the tab index. Pass `-1` to take the editor out of the Tab
    /// focus chain (mouse-only); `0` to put it back in normal flow.
    pub fn set_tabindex(&self, n: i32) {
        let _ = self.root.set_attribute("tabindex", &n.to_string());
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
            Selection::Cell { anchor, head } => {
                // Render as a contiguous range covering both cells. A
                // browser Range can't paint a true rectangular cell
                // selection; the editor highlights cells via decorations.
                let lo = anchor.min(head);
                let hi = anchor.max(head);
                let hi_end = self
                    .doc
                    .node_at(hi)
                    .map(|n| hi + n.node_size())
                    .unwrap_or(hi);
                (lo, hi_end)
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
            Selection::Cell { anchor, head } => {
                let lo = anchor.min(head);
                let hi = anchor.max(head);
                let hi_end = self
                    .doc
                    .node_at(hi)
                    .map(|n| hi + n.node_size())
                    .unwrap_or(hi);
                (lo, hi_end)
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

    /// Paste Markdown at the current DOM selection. The text is parsed
    /// through [`taino_edit_core::markdown::parse_markdown`] and validated
    /// against the schema, so unknown constructs are dropped rather than
    /// breaking the doc. Adapters prefer this over `paste_text` when the
    /// clipboard advertises `text/markdown`.
    pub fn paste_markdown(&self, md: &str) -> Option<Transform> {
        let parsed = taino_edit_core::markdown::parse_markdown(&self.schema, md).ok()?;
        let (from, to) = self.paste_range()?;
        let slice = Slice::new(parsed.content().clone(), 0, 0);
        let mut transform = Transform::new(self.doc.clone());
        transform.replace(from, to, slice, &self.schema).ok()?;
        Some(transform)
    }

    /// Extract a [`Slice`] of the document between `from` and `to` — what
    /// adapters dispatch as the "dragged content" on `dragstart`. Returns
    /// `None` if the range is out of bounds.
    pub fn extract_slice(&self, from: usize, to: usize) -> Option<Slice> {
        self.doc.slice(from, to).ok()
    }

    /// Insert `slice` at document position `at`, producing the
    /// [`Transform`] that commits the drop. Returns `None` when the
    /// resulting doc would violate the schema (e.g. dropping a block into
    /// inline content).
    pub fn drop_slice(&self, slice: &Slice, at: usize) -> Option<Transform> {
        let mut transform = Transform::new(self.doc.clone());
        transform
            .replace(at, at, slice.clone(), &self.schema)
            .ok()?;
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
            if let Some(dom) = dom_element_at(children, 0, *pos) {
                let list = dom.class_list();
                if add {
                    let _ = list.add_1(class);
                } else {
                    let _ = list.remove_1(class);
                }
            }
        }
        // Inline decorations are drawn in the overlay layer, not by toggling
        // a class on document DOM — see `EditorView::render_inline_overlay`.
        Decoration::Inline { .. } => {}
    }
}

/// The document position directly before the node whose DOM element is
/// `target`, searched recursively. `None` if `target` isn't a node element
/// in the view tree.
fn pos_before_element(children: &[ViewDesc], base: usize, target: &Element) -> Option<usize> {
    let mut pos = base;
    for c in children {
        match c {
            ViewDesc::Text { node, .. } => {
                pos += node.node_size();
            }
            ViewDesc::Element {
                node,
                dom,
                children: kids,
            } => {
                if dom.is_same_node(Some(target.as_ref())) {
                    return Some(pos);
                }
                if let Some(p) = pos_before_element(kids, pos + 1, target) {
                    return Some(p);
                }
                pos += node.node_size();
            }
        }
    }
    None
}

/// The DOM element of the node that begins exactly at `target`, searched
/// recursively through element descriptors. `base` is the document position
/// just inside the parent of `children`. Returns the element for nested
/// nodes too (e.g. a table cell), not just top-level blocks.
fn dom_element_at(children: &[ViewDesc], base: usize, target: usize) -> Option<&Element> {
    let mut pos = base;
    for c in children {
        match c {
            ViewDesc::Text { node, .. } => {
                pos += node.node_size();
            }
            ViewDesc::Element {
                node,
                dom,
                children: kids,
            } => {
                if pos == target {
                    return Some(dom);
                }
                let size = node.node_size();
                if target > pos && target < pos + size {
                    if let Some(e) = dom_element_at(kids, pos + 1, target) {
                        return Some(e);
                    }
                }
                pos += size;
            }
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
