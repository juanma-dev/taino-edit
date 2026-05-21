//! `align` — `text_align` attribute on `paragraph` / `heading`, plus four
//! `align_*` commands.
//!
//! The attribute itself is declared by [`Paragraph`](crate::Paragraph) and
//! [`Heading`](crate::Heading) (they both reference [`text_align_attr_spec`]
//! and the helpers in this module from their specs). The `Align` extension
//! only contributes the keymap bindings; the underlying commands are
//! exported so toolbar buttons can call them directly.

use taino_edit_core::{
    AttrSpec, AttrValue, Attrs, Command, HtmlElement, ResolvedPos, Schema, Selection,
};

use crate::{Extension, SchemaAdditions};

/// The block alignments the Align extension knows about.
const ALIGNMENTS: &[&str] = &["left", "center", "right", "justify"];

/// The `AttrSpec` to declare on any block node that should support alignment.
/// Default is `null` (no `style` attribute emitted, identical HTML to today).
pub fn text_align_attr_spec() -> AttrSpec {
    AttrSpec {
        default: Some(AttrValue::Null),
    }
}

/// Parse a `style="text-align: …"` attribute off an `HtmlElement` into an
/// `Attrs` map carrying `text_align`. Returns an empty map if the style is
/// missing/unrecognized — the caller then merges in its own (level, …)
/// attrs.
pub fn parse_align_attrs(el: &HtmlElement) -> Attrs {
    let mut a = Attrs::new();
    if let Some(style) = el.attr("style") {
        let lower = style.to_ascii_lowercase();
        for align in ALIGNMENTS {
            if lower.contains(&format!("text-align: {align}"))
                || lower.contains(&format!("text-align:{align}"))
            {
                a.insert(
                    "text_align".to_string(),
                    AttrValue::from((*align).to_string()),
                );
                break;
            }
        }
    }
    a
}

/// If `attrs` carries a non-null `text_align`, render it as a `style`
/// attribute value. Returns `None` for the default (null) so the
/// serializer omits the attribute entirely.
pub fn align_attrs_for_dom(attrs: &Attrs) -> Option<String> {
    let v = attrs.get("text_align")?.as_str()?;
    Some(format!("text-align: {v}"))
}

/// The alignment extension. Binds `Mod-Shift-L/E/R/J` to the four
/// alignments. Block nodes opt in by declaring a `text_align` attr (the
/// shipped `Paragraph` and `Heading` extensions already do).
pub struct Align;

impl Extension for Align {
    fn name(&self) -> &str {
        "align"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        // The attribute itself is declared by the block nodes; nothing to add.
        SchemaAdditions::default()
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        vec![
            ("Mod-Shift-l".to_string(), align_left()),
            ("Mod-Shift-e".to_string(), align_center()),
            ("Mod-Shift-r".to_string(), align_right()),
            ("Mod-Shift-j".to_string(), align_justify()),
        ]
    }
}

fn set_align(value: Option<&'static str>) -> Command {
    Box::new(move |state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        if rp.depth() == 0 {
            return false;
        }
        // Find the outermost (depth-1) enclosing block — set_block_type
        // operates on the same target so toolbar + keymap stay consistent.
        let block_pos = rp.before(1);
        let block = rp.node(1);
        if !block.node_type().spec().attrs.contains_key("text_align") {
            return false; // this block type does not support alignment
        }
        let new_value = match value {
            Some(v) => AttrValue::from(v.to_string()),
            None => AttrValue::Null,
        };
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            if tx
                .transform()
                .set_node_attr(block_pos, "text_align", new_value, state.schema())
                .is_ok()
            {
                // Preserve the selection — the doc size doesn't change.
                tx.set_selection(state.selection());
                d(tx);
            }
        }
        true
    })
}

/// Reset alignment to the schema default (`null`, i.e. browser default).
pub fn align_left() -> Command {
    set_align(Some("left"))
}
/// Center-align the enclosing block.
pub fn align_center() -> Command {
    set_align(Some("center"))
}
/// Right-align the enclosing block.
pub fn align_right() -> Command {
    set_align(Some("right"))
}
/// Justify-align the enclosing block.
pub fn align_justify() -> Command {
    set_align(Some("justify"))
}

// Re-exported for the demo: a no-op anchor so wildcard imports of
// `taino_edit_extensions::*` pick up the selection type (used by the
// alignment command's `set_selection` call).
#[allow(dead_code)]
fn _selection_in_scope(_: &Selection) {}
