//! `bold` — the `strong` mark, toggled via `Mod-b`.

use taino_edit_core::{toggle_mark, Command, DomSpec, MarkSpec, ParseRule, Schema};

use crate::{Extension, SchemaAdditions};

/// The bold extension. Adds the `strong` mark and binds `Mod-b` to
/// [`toggle_mark`].
pub struct Bold;

impl Extension for Bold {
    fn name(&self) -> &str {
        "bold"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions {
            marks: vec![(
                "strong".to_string(),
                MarkSpec {
                    to_dom: Some(|_| DomSpec::element("strong")),
                    parse_dom: vec![ParseRule::tag("strong"), ParseRule::tag("b")],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, schema: &Schema) -> Vec<(String, Command)> {
        let Some(mt) = schema.mark_type("strong") else {
            return Vec::new();
        };
        vec![("Mod-b".to_string(), toggle_mark(mt.clone()))]
    }
}
