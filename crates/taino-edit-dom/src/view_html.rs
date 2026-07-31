//! Host-safe serialization of a document to the **exact markup the view
//! renders** — the SSR / first-paint counterpart of `view::render`.
//!
//! [`Node::to_html`](taino_edit_core::Node::to_html) in `core` serializes the
//! *document*; this module serializes what the *editor* shows for it, which
//! differs in three deliberate ways (each mirroring `view.rs`):
//!
//! 1. **Empty textblocks get a trailing `<br
//!    data-taino-trailing-break="">`** — a bare `<p></p>` is zero-height in
//!    the browser, so without it a server-rendered empty document would
//!    visually "grow" the moment the editor mounts.
//! 2. **Transparent nodes (`to_dom: None`) render as `<span>`** — the view
//!    needs a real container to edit inside; the serializer must agree.
//! 3. **Void vs. container emission follows the browser's own rules** — the
//!    view builds elements with `create_element`, so nothing is ever truly
//!    self-closing; we emit `<img …>` for HTML void elements and an explicit
//!    `<tag …></tag>` for everything else, matching what `innerHTML` would
//!    read back. (Never `<tag/>`: the HTML parser ignores the slash, and a
//!    literal `</br>` close tag would even parse as a *second* `<br>`.)
//!
//! The contract "serializer output parses to the DOM `render` builds" is
//! pinned by a browser test in `tests/view_html_contract.rs` that compares
//! this output — parsed and re-serialized by the browser — against a mounted
//! editor's `innerHTML`, plus host tests below for each divergence.
//!
//! This module is `web-sys`-free on purpose: it is what a server (Leptos SSR)
//! calls to pre-render the initial document, where no DOM exists.

use taino_edit_core::{DomSpec, Node};

/// Marker attribute on the synthetic trailing `<br>` the view adds to empty
/// textblocks so the caret can land in them (a bare `<p></p>` is zero-height
/// and unfocusable in `contenteditable`). Lets us find/remove only *our*
/// break and skip it when reading text back.
pub(crate) const TRAILING_BREAK_ATTR: &str = "data-taino-trailing-break";

/// A block node that holds *inline* content (paragraph, heading, code block,
/// …) — the nodes that need a trailing break when empty. Block *containers*
/// (doc, blockquote, list, list item, table cell) hold other blocks and must
/// **not** be treated as textblocks: doing so would (a) add stray breaks and
/// (b) make `reconcile_trailing_break` strip a nested block's break. We
/// detect inline content from the content expression (`inline*`, `text*`, …).
pub(crate) fn is_textblock(node: &Node) -> bool {
    node.node_type().is_block()
        && node
            .node_type()
            .spec()
            .content
            .as_deref()
            .is_some_and(|c| c.contains("inline") || c.contains("text"))
}

/// The HTML void elements — tags the browser never gives children or a
/// closing tag. Emission must match `innerHTML` serialization for the
/// mount-parity contract to hold.
const VOID_TAGS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Serialize `doc`'s children to the markup [`EditorView::mount`] would
/// build for them — the string to server-render (or first-paint) inside the
/// editor's container element.
///
/// Like the view, this renders the document node itself transparently: the
/// container element plays the role of the doc, its children are the doc's
/// blocks.
///
/// The output is safe to inject as raw HTML (e.g. Leptos's `inner_html`):
/// tags and attribute names come only from the schema's [`DomSpec`]s, and
/// text plus attribute values are HTML-escaped here — there is no path for
/// document text to smuggle markup through.
///
/// [`EditorView::mount`]: crate::EditorView::mount
pub fn doc_view_html(doc: &Node) -> String {
    doc.content().iter().map(node_view_html).collect()
}

/// One node, rendered exactly as `view::render` / `view::render_text` would.
fn node_view_html(node: &Node) -> String {
    // Text: the raw (escaped) text, wrapped by its marks' elements —
    // iteration order wraps outward, so the *last* mark is outermost,
    // mirroring `render_text`.
    if let Some(text) = node.text() {
        let mut s = escape_text(text);
        for mark in node.marks() {
            if let Some(f) = mark.mark_type().spec().to_dom {
                let spec = f(mark);
                s = format!("{}>{}</{}>", open_tag(&spec), s, spec.tag());
            }
        }
        return s;
    }

    let children: String = node.content().iter().map(node_view_html).collect();

    let Some(f) = node.node_type().spec().to_dom else {
        // Transparent node: `render` still creates a container so editing
        // inside it works; `<span>` is its conservative default.
        return format!("<span>{children}</span>");
    };
    let spec = f(node);

    if VOID_TAGS.contains(&spec.tag()) {
        return format!("{}>", open_tag(&spec));
    }

    let trailing_break = if node.child_count() == 0 && is_textblock(node) {
        format!("<br {TRAILING_BREAK_ATTR}=\"\">")
    } else {
        String::new()
    };
    format!(
        "{}>{}{}</{}>",
        open_tag(&spec),
        children,
        trailing_break,
        spec.tag()
    )
}

// The escaping below intentionally matches `core::html` (`Node::to_html`);
// the `matches_core_to_html_for_plain_content` test pins the two together.

fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// `<tag` plus its attributes, in `DomSpec` order — the same order
/// `create_element` applies them, so browser read-back agrees.
fn open_tag(spec: &DomSpec) -> String {
    let mut s = String::new();
    s.push('<');
    s.push_str(spec.tag());
    for (k, v) in spec.attrs() {
        s.push(' ');
        s.push_str(k);
        s.push_str("=\"");
        s.push_str(&escape_attr(v));
        s.push('"');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use taino_edit_core::{AttrSpec, MarkSpec, NodeSpec, Schema, SchemaBuilder};

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
                                default: Some(serde_json::json!(1)),
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
                    // No `to_dom`: the transparent-node case.
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

    fn doc(s: &Schema, children: Vec<Node>) -> Node {
        s.node("doc", Default::default(), children, vec![]).unwrap()
    }

    fn para(s: &Schema, children: Vec<Node>) -> Node {
        s.node("paragraph", Default::default(), children, vec![])
            .unwrap()
    }

    #[test]
    fn plain_paragraph_and_escaping() {
        let s = schema();
        let d = doc(
            &s,
            vec![para(&s, vec![s.text("a <b> & c", vec![]).unwrap()])],
        );
        assert_eq!(doc_view_html(&d), "<p>a &lt;b&gt; &amp; c</p>");
    }

    #[test]
    fn empty_textblock_gets_the_views_trailing_break() {
        let s = schema();
        let d = doc(&s, vec![para(&s, vec![])]);
        assert_eq!(
            doc_view_html(&d),
            format!("<p><br {TRAILING_BREAK_ATTR}=\"\"></p>")
        );
    }

    #[test]
    fn marks_wrap_with_the_last_mark_outermost() {
        let s = schema();
        let bold = s.mark_type("bold").unwrap().create(Default::default());
        let em = s.mark_type("em").unwrap().create(Default::default());
        let d = doc(
            &s,
            vec![para(&s, vec![s.text("x", vec![bold, em]).unwrap()])],
        );
        // Same nesting `render_text` builds: later mark = outer element.
        assert_eq!(doc_view_html(&d), "<p><em><strong>x</strong></em></p>");
    }

    #[test]
    fn heading_attrs_and_void_image() {
        let s = schema();
        let mut attrs = std::collections::BTreeMap::new();
        attrs.insert("level".into(), serde_json::json!(2));
        let h = s
            .node("heading", attrs, vec![s.text("T", vec![]).unwrap()], vec![])
            .unwrap();
        let mut iattrs = std::collections::BTreeMap::new();
        iattrs.insert("src".into(), serde_json::json!("a\"b.png"));
        let img = s.node("image", iattrs, vec![], vec![]).unwrap();
        let d = doc(&s, vec![h, para(&s, vec![img])]);
        assert_eq!(
            doc_view_html(&d),
            "<h2>T</h2><p><img src=\"a&quot;b.png\"></p>"
        );
    }

    #[test]
    fn transparent_node_renders_as_span_like_the_view() {
        let s = schema();
        let inner = para(&s, vec![s.text("in", vec![]).unwrap()]);
        let ghost = s
            .node("ghost", Default::default(), vec![inner], vec![])
            .unwrap();
        let d = doc(&s, vec![ghost]);
        assert_eq!(doc_view_html(&d), "<span><p>in</p></span>");
    }

    /// For content with no empty textblocks, no transparent nodes, and no
    /// void elements, the view markup and `Node::to_html` must be the same
    /// string — this pins our escaping/attr-order to `core`'s.
    #[test]
    fn matches_core_to_html_for_plain_content() {
        let s = schema();
        let bold = s.mark_type("bold").unwrap().create(Default::default());
        let d = doc(
            &s,
            vec![
                para(&s, vec![s.text("a & b", vec![bold]).unwrap()]),
                para(&s, vec![s.text("plain", vec![]).unwrap()]),
            ],
        );
        assert_eq!(doc_view_html(&d), d.to_html());
    }
}
