//! `transform-case` — `to_uppercase` / `to_lowercase` commands over the
//! current selection.
//!
//! No schema additions and no keymap bindings (case shortcuts vary too much
//! across keyboards — `Mod-U` collides with the browser "View source" on
//! several layouts). The host wires the buttons it wants.

use taino_edit_core::{Command, Fragment, Mark, Node, Schema, Slice};

use crate::{Extension, SchemaAdditions};

/// The case-transform extension. Carries no schema or keymap; only re-exports
/// [`to_uppercase`] / [`to_lowercase`] as the canonical commands.
pub struct TransformCase;

impl Extension for TransformCase {
    fn name(&self) -> &str {
        "transform-case"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions::default()
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        Vec::new()
    }
}

fn map_text_nodes(n: &Node, schema: &Schema, f: &dyn Fn(&str) -> String) -> Node {
    if let Some(t) = n.text() {
        // Marks survive; only the text changes.
        return schema
            .text(&f(t), n.marks().to_vec())
            .unwrap_or_else(|_| n.clone());
    }
    let kids: Vec<Node> = n
        .content()
        .iter()
        .map(|c| map_text_nodes(c, schema, f))
        .collect();
    let frag = Fragment::from_nodes(kids);
    // Rebuild the wrapper via the schema so content validation runs (the
    // text only changes, so the same children always satisfy the parent's
    // content expression).
    schema
        .node(
            n.node_type().name(),
            n.attrs().clone(),
            frag.children().to_vec(),
            n.marks().to_vec(),
        )
        .unwrap_or_else(|_| n.clone())
}

fn map_slice(slice: &Slice, schema: &Schema, f: &dyn Fn(&str) -> String) -> Slice {
    let kids: Vec<Node> = slice
        .content()
        .iter()
        .map(|c| map_text_nodes(c, schema, f))
        .collect();
    Slice::new(
        Fragment::from_nodes(kids),
        slice.open_start(),
        slice.open_end(),
    )
}

fn case_command(f: fn(&str) -> String) -> Command {
    Box::new(move |state, dispatch| {
        let sel = state.selection();
        let (from, to) = (sel.from(), sel.to(state.doc()));
        if from >= to {
            return false;
        }
        let Ok(slice) = state.doc().slice(from, to) else {
            return false;
        };
        // Pure-text slices are the only ones that *might* not change; for
        // those an unchanged result short-circuits.
        let new_slice = map_slice(&slice, state.schema(), &f);
        if same_text(&slice, &new_slice) {
            return false;
        }
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            if tx
                .transform()
                .replace(from, to, new_slice, state.schema())
                .is_ok()
            {
                d(tx);
            }
        }
        true
    })
}

fn same_text(a: &Slice, b: &Slice) -> bool {
    fn text(n: &Node, out: &mut String) {
        if let Some(t) = n.text() {
            out.push_str(t);
        } else {
            for c in n.content().iter() {
                text(c, out);
            }
        }
    }
    let mut ta = String::new();
    let mut tb = String::new();
    for c in a.content().iter() {
        text(c, &mut ta);
    }
    for c in b.content().iter() {
        text(c, &mut tb);
    }
    ta == tb
}

/// Uppercase every text run inside the current selection (marks preserved).
pub fn to_uppercase() -> Command {
    case_command(str_to_upper)
}

/// Lowercase every text run inside the current selection (marks preserved).
pub fn to_lowercase() -> Command {
    case_command(str_to_lower)
}

fn str_to_upper(s: &str) -> String {
    s.to_uppercase()
}
fn str_to_lower(s: &str) -> String {
    s.to_lowercase()
}

// Suppress dead-code warnings for the Mark re-export pulled in for future
// command variants (e.g. case-toggle-respecting-marks).
#[allow(dead_code)]
fn _mark_in_scope(_: &Mark) {}
