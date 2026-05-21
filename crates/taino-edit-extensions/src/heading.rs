//! `heading` — `h1`/`h2`/`h3` blocks with a `level` attr, bound to
//! `Mod-Alt-1..3`.

use std::collections::HashMap;

use taino_edit_core::{
    set_block_type, AttrSpec, AttrValue, Attrs, Command, DomSpec, HtmlElement, NodeSpec, ParseRule,
    Schema,
};

use crate::{Extension, SchemaAdditions};

/// The heading extension. Adds the `heading` node (with a `level` attr,
/// default 1) and binds `Mod-Alt-1` / `Mod-Alt-2` / `Mod-Alt-3` to
/// [`set_block_type`] with the matching level.
pub struct Heading;

fn h1_attrs(_: &HtmlElement) -> Option<Attrs> {
    Some(level_attrs(1))
}
fn h2_attrs(_: &HtmlElement) -> Option<Attrs> {
    Some(level_attrs(2))
}
fn h3_attrs(_: &HtmlElement) -> Option<Attrs> {
    Some(level_attrs(3))
}

fn level_attrs(level: u64) -> Attrs {
    let mut a = Attrs::new();
    a.insert("level".to_string(), AttrValue::from(level));
    a
}

impl Extension for Heading {
    fn name(&self) -> &str {
        "heading"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        let mut attrs = HashMap::new();
        attrs.insert(
            "level".to_string(),
            AttrSpec {
                default: Some(AttrValue::from(1u64)),
            },
        );
        SchemaAdditions {
            nodes: vec![(
                "heading".to_string(),
                NodeSpec {
                    content: Some("inline*".into()),
                    group: Some("block".into()),
                    attrs,
                    to_dom: Some(|n| {
                        let level = n.attrs().get("level").and_then(|v| v.as_u64()).unwrap_or(1);
                        DomSpec::element(&format!("h{level}"))
                    }),
                    parse_dom: vec![
                        ParseRule::with_attrs("h1", h1_attrs),
                        ParseRule::with_attrs("h2", h2_attrs),
                        ParseRule::with_attrs("h3", h3_attrs),
                    ],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        vec![
            (
                "Mod-Alt-1".to_string(),
                set_block_type("heading", level_attrs(1)),
            ),
            (
                "Mod-Alt-2".to_string(),
                set_block_type("heading", level_attrs(2)),
            ),
            (
                "Mod-Alt-3".to_string(),
                set_block_type("heading", level_attrs(3)),
            ),
        ]
    }
}
