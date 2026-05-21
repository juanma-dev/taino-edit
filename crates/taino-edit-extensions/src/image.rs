//! `image` — an inline `<img>` atom with `src` / `alt` / `title` attrs.

use std::collections::HashMap;

use taino_edit_core::{
    AttrSpec, AttrValue, Attrs, Command, DomSpec, Fragment, HtmlElement, NodeSpec, ParseRule,
    Selection, Slice,
};

use crate::{Extension, SchemaAdditions};

/// The image extension. Adds the `image` inline leaf node. The [`insert_image`]
/// command is exported for the host to wire to a UI (a file picker, prompt,
/// drag-and-drop handler, …).
pub struct Image;

fn image_attrs(el: &HtmlElement) -> Option<Attrs> {
    let src = el.attr("src")?.to_string();
    let mut a = Attrs::new();
    a.insert("src".to_string(), AttrValue::from(src));
    let alt = el.attr("alt").unwrap_or("").to_string();
    a.insert("alt".to_string(), AttrValue::from(alt));
    if let Some(title) = el.attr("title") {
        a.insert("title".to_string(), AttrValue::from(title.to_string()));
    } else {
        a.insert("title".to_string(), AttrValue::Null);
    }
    Some(a)
}

impl Extension for Image {
    fn name(&self) -> &str {
        "image"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        let mut attrs = HashMap::new();
        attrs.insert(
            "src".to_string(),
            AttrSpec {
                default: Some(AttrValue::from(String::new())),
            },
        );
        attrs.insert(
            "alt".to_string(),
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
            nodes: vec![(
                "image".to_string(),
                NodeSpec {
                    group: Some("inline".into()),
                    inline: true,
                    atom: true,
                    attrs,
                    to_dom: Some(|n| {
                        let src = n.attrs().get("src").and_then(|v| v.as_str()).unwrap_or("");
                        let alt = n.attrs().get("alt").and_then(|v| v.as_str()).unwrap_or("");
                        let mut spec = DomSpec::void("img").attr("src", src).attr("alt", alt);
                        if let Some(t) = n.attrs().get("title").and_then(|v| v.as_str()) {
                            spec = spec.attr("title", t.to_string());
                        }
                        spec
                    }),
                    parse_dom: vec![ParseRule::with_attrs("img", image_attrs)],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }
}

/// Insert an `image` node at the current selection. If the selection covers a
/// range it is replaced; otherwise the image is inserted at the caret. The
/// caret is placed just after the image.
pub fn insert_image(src: impl Into<String>, alt: Option<String>) -> Command {
    let src = src.into();
    Box::new(move |state, dispatch| {
        let Some(_) = state.schema().node_type("image") else {
            return false;
        };
        let sel = state.selection();
        let (from, to) = (sel.from(), sel.to(state.doc()));
        let mut attrs = Attrs::new();
        attrs.insert("src".to_string(), AttrValue::from(src.clone()));
        if let Some(a) = &alt {
            attrs.insert("alt".to_string(), AttrValue::from(a.clone()));
        }
        let Ok(img) = state.schema().node("image", attrs, vec![], vec![]) else {
            return false;
        };
        let slice = Slice::new(Fragment::from_node(img), 0, 0);
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            if tx
                .transform()
                .replace(from, to, slice, state.schema())
                .is_ok()
            {
                // After replace, caret sits one position past the inserted
                // image's start (atom size = 1).
                tx.set_selection(Selection::caret(from + 1));
                d(tx);
            }
        }
        true
    })
}
