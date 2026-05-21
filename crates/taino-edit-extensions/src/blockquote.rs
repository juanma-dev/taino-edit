//! `blockquote` — a `<blockquote>` block wrapping one or more block
//! children, bound to `Mod->`.

use taino_edit_core::{wrap_in, Attrs, Command, DomSpec, NodeSpec, ParseRule, Schema};

use crate::{Extension, SchemaAdditions};

/// The blockquote extension. Adds the `blockquote` node and binds `Mod->`
/// to [`wrap_in`] for it.
pub struct Blockquote;

impl Extension for Blockquote {
    fn name(&self) -> &str {
        "blockquote"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions {
            nodes: vec![(
                "blockquote".to_string(),
                NodeSpec {
                    content: Some("block+".into()),
                    group: Some("block".into()),
                    to_dom: Some(|_| DomSpec::element("blockquote")),
                    parse_dom: vec![ParseRule::tag("blockquote")],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        vec![("Mod->".to_string(), wrap_in("blockquote", Attrs::new()))]
    }
}
