//! `italic` — the `em` mark, toggled via `Mod-i`.

use taino_edit_core::{toggle_mark, Command, DomSpec, MarkSpec, ParseRule, Schema};

use crate::{Extension, SchemaAdditions};

/// The italic extension. Adds the `em` mark and binds `Mod-i` to
/// [`toggle_mark`].
pub struct Italic;

impl Extension for Italic {
    fn name(&self) -> &str {
        "italic"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions {
            marks: vec![(
                "em".to_string(),
                MarkSpec {
                    to_dom: Some(|_| DomSpec::element("em")),
                    parse_dom: vec![ParseRule::tag("em"), ParseRule::tag("i")],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, schema: &Schema) -> Vec<(String, Command)> {
        let Some(mt) = schema.mark_type("em") else {
            return Vec::new();
        };
        vec![("Mod-i".to_string(), toggle_mark(mt.clone()))]
    }
}
