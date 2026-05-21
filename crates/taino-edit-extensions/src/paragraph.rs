//! `paragraph` — the canonical text block, bound to `Mod-Alt-0`.

use taino_edit_core::{set_block_type, Attrs, Command, DomSpec, NodeSpec, ParseRule, Schema};

use crate::{Extension, SchemaAdditions};

/// The paragraph extension. Adds the `paragraph` node and binds
/// `Mod-Alt-0` to [`set_block_type`] for it.
pub struct Paragraph;

impl Extension for Paragraph {
    fn name(&self) -> &str {
        "paragraph"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions {
            nodes: vec![(
                "paragraph".to_string(),
                NodeSpec {
                    content: Some("inline*".into()),
                    group: Some("block".into()),
                    to_dom: Some(|_| DomSpec::element("p")),
                    parse_dom: vec![ParseRule::tag("p")],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        vec![(
            "Mod-Alt-0".to_string(),
            set_block_type("paragraph", Attrs::new()),
        )]
    }
}
