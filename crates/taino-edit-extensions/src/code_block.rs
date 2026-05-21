//! `code-block` — a `<pre>` block holding raw text, bound to `` Mod-` ``.

use taino_edit_core::{set_block_type, Attrs, Command, DomSpec, NodeSpec, ParseRule, Schema};

use crate::{Extension, SchemaAdditions};

/// The code-block extension. Adds the `code_block` node and binds the
/// backtick key (`` Mod-` ``) to `set_block_type("code_block")`.
pub struct CodeBlock;

impl Extension for CodeBlock {
    fn name(&self) -> &str {
        "code_block"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions {
            nodes: vec![(
                "code_block".to_string(),
                NodeSpec {
                    content: Some("text*".into()),
                    group: Some("block".into()),
                    marks: Some(String::new()), // no marks inside code
                    to_dom: Some(|_| DomSpec::element("pre")),
                    parse_dom: vec![ParseRule::tag("pre")],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        vec![(
            "Mod-`".to_string(),
            set_block_type("code_block", Attrs::new()),
        )]
    }
}
