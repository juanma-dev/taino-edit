//! `code` — the inline code mark (`<code>`), toggled via `Mod-e`.
//!
//! Distinct from [`CodeBlock`](crate::CodeBlock) (a `<pre>` block): this is
//! an inline mark for code spans within a paragraph, and it round-trips to
//! Markdown backticks (`` `like this` ``).

use taino_edit_core::{toggle_mark, Command, DomSpec, MarkSpec, ParseRule, Schema};

use crate::{Extension, SchemaAdditions};

/// The inline-code extension. Adds the `code` mark and binds `Mod-e` to
/// [`toggle_mark`].
pub struct Code;

impl Extension for Code {
    fn name(&self) -> &str {
        "code"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions {
            marks: vec![(
                "code".to_string(),
                MarkSpec {
                    // Code spans don't extend when typing at their edge.
                    inclusive: false,
                    to_dom: Some(|_| DomSpec::element("code")),
                    parse_dom: vec![ParseRule::tag("code")],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, schema: &Schema) -> Vec<(String, Command)> {
        let Some(mt) = schema.mark_type("code") else {
            return Vec::new();
        };
        vec![("Mod-e".to_string(), toggle_mark(mt.clone()))]
    }
}
