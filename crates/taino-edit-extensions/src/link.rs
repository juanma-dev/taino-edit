//! `link` — an `<a href=…>` mark over a text range.
//!
//! Adds the `link` mark (with `href` and optional `title` attrs), a
//! [`set_link`] command that applies it to the selection, a [`remove_link`]
//! command that strips it, and the standard `Mod-k` binding (a no-op until
//! the host supplies a URL, since `core` is dependency-free and may not
//! call `window.prompt`).

use std::collections::HashMap;

use taino_edit_core::{
    AttrSpec, AttrValue, Attrs, Command, DomSpec, HtmlElement, Mark, MarkSpec, Node, ParseRule,
    Schema,
};

use crate::{Extension, SchemaAdditions};

/// The link extension. Adds the `link` mark; commands [`set_link`] and
/// [`remove_link`] are exported so the host can wire them to a UI prompt.
/// A `Mod-k` binding is left for the host to add when it knows how to ask
/// for a URL (the demo wires it via `window.prompt`).
pub struct Link;

fn link_attrs(el: &HtmlElement) -> Option<Attrs> {
    let href = el.attr("href")?.to_string();
    let mut a = Attrs::new();
    a.insert("href".to_string(), AttrValue::from(href));
    if let Some(title) = el.attr("title") {
        a.insert("title".to_string(), AttrValue::from(title.to_string()));
    }
    Some(a)
}

impl Extension for Link {
    fn name(&self) -> &str {
        "link"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        let mut attrs = HashMap::new();
        attrs.insert(
            "href".to_string(),
            AttrSpec {
                default: Some(AttrValue::from(String::new())),
            },
        );
        attrs.insert(
            "title".to_string(),
            AttrSpec {
                default: Some(AttrValue::Null),
            },
        );
        SchemaAdditions {
            marks: vec![(
                "link".to_string(),
                MarkSpec {
                    inclusive: false,
                    attrs,
                    to_dom: Some(|m| {
                        let href = m
                            .attrs()
                            .get("href")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let mut spec = DomSpec::element("a").attr("href", href);
                        if let Some(title) = m.attrs().get("title").and_then(|v| v.as_str()) {
                            spec = spec.attr("title", title.to_string());
                        }
                        spec
                    }),
                    parse_dom: vec![ParseRule::with_attrs("a", link_attrs)],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        // `Mod-k` is not bound here because applying a link requires asking
        // the user for a URL, and `taino-edit-extensions` is dependency-free
        // (no `web-sys`). The demo wires the binding through `set_link`.
        Vec::new()
    }
}

/// Collect every distinct `link`-typed mark instance in `doc[from..to]`.
/// `RemoveMarkStep` matches by (type, attrs), so we have to enumerate the
/// concrete attribute combinations actually present.
fn link_marks_in(doc: &Node, from: usize, to: usize) -> Vec<Mark> {
    let Ok(slice) = doc.slice(from, to) else {
        return Vec::new();
    };
    let mut out: Vec<Mark> = Vec::new();
    fn walk(n: &Node, out: &mut Vec<Mark>) {
        if n.is_inline() {
            for m in n.marks() {
                if m.mark_type().name() == "link" && !out.iter().any(|e| e == m) {
                    out.push(m.clone());
                }
            }
        }
        for c in n.content().iter() {
            walk(c, out);
        }
    }
    for c in slice.content().iter() {
        walk(c, &mut out);
    }
    out
}

/// Wrap the current selection with a `link` mark carrying `href` (and the
/// optional `title`). A no-op when the selection is empty.
pub fn set_link(href: impl Into<String>, title: Option<String>) -> Command {
    let href = href.into();
    Box::new(move |state, dispatch| {
        let sel = state.selection();
        let (from, to) = (sel.from(), sel.to(state.doc()));
        if from >= to {
            return false;
        }
        let Some(mt) = state.schema().mark_type("link") else {
            return false;
        };
        let mut attrs = Attrs::new();
        attrs.insert("href".to_string(), AttrValue::from(href.clone()));
        if let Some(t) = &title {
            attrs.insert("title".to_string(), AttrValue::from(t.clone()));
        }
        let mark = mt.create(attrs);
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            // Strip any existing link marks on this range first so href
            // edits don't stack into the same text run.
            for m in link_marks_in(state.doc(), from, to) {
                let _ = tx.transform().remove_mark(from, to, m, state.schema());
            }
            if tx
                .transform()
                .add_mark(from, to, mark, state.schema())
                .is_ok()
            {
                d(tx);
            }
        }
        true
    })
}

/// Strip every `link` mark on the current selection.
pub fn remove_link() -> Command {
    Box::new(|state, dispatch| {
        let sel = state.selection();
        let (from, to) = (sel.from(), sel.to(state.doc()));
        if from >= to {
            return false;
        }
        let marks = link_marks_in(state.doc(), from, to);
        if marks.is_empty() {
            return false;
        }
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            for m in marks {
                let _ = tx.transform().remove_mark(from, to, m, state.schema());
            }
            d(tx);
        }
        true
    })
}
