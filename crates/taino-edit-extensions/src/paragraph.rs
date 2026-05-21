//! `paragraph` — the canonical text block, bound to `Mod-Alt-0`.

use std::collections::HashMap;

use taino_edit_core::{
    set_block_type, AttrSpec, AttrValue, Attrs, Command, DomSpec, HtmlElement, NodeSpec, ParseRule,
    Schema,
};

use crate::align::{align_attrs_for_dom, parse_align_attrs, text_align_attr_spec};
use crate::{Extension, SchemaAdditions};

/// The paragraph extension. Adds the `paragraph` node and binds
/// `Mod-Alt-0` to [`set_block_type`] for it.
pub struct Paragraph;

fn paragraph_attrs(el: &HtmlElement) -> Option<Attrs> {
    Some(parse_align_attrs(el))
}

impl Extension for Paragraph {
    fn name(&self) -> &str {
        "paragraph"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        let mut attrs: HashMap<String, AttrSpec> = HashMap::new();
        attrs.insert("text_align".to_string(), text_align_attr_spec());
        SchemaAdditions {
            nodes: vec![(
                "paragraph".to_string(),
                NodeSpec {
                    content: Some("inline*".into()),
                    group: Some("block".into()),
                    attrs,
                    to_dom: Some(|n| {
                        let mut spec = DomSpec::element("p");
                        if let Some(style) = align_attrs_for_dom(n.attrs()) {
                            spec = spec.attr("style", style);
                        }
                        spec
                    }),
                    parse_dom: vec![ParseRule::with_attrs("p", paragraph_attrs)],
                    ..Default::default()
                },
            )],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        // Preserve text_align by reading the current block's attr — but the
        // simple Mod-Alt-0 binding here resets to a plain paragraph; the
        // alignment commands in `align` set the attr directly via AttrStep.
        let _ = AttrValue::Null; // silence the unused-import warning in some builds
        vec![(
            "Mod-Alt-0".to_string(),
            set_block_type("paragraph", Attrs::new()),
        )]
    }
}
