//! Lists — `bullet_list`, `ordered_list` and `list_item` nodes, plus
//! wrap/lift commands.
//!
//! v0.1 ships the schema + the canonical wrap commands so users can build
//! bulleted and numbered lists. Smart Enter / nested list indentation
//! (`split_list_item`, `sink_list_item`) are deferred to v0.2 — the host
//! can still emulate them by composing existing commands.

use std::collections::HashMap;

use taino_edit_core::{
    AttrSpec, AttrValue, Attrs, Command, DomSpec, Fragment, HtmlElement, NodeSpec, ParseRule,
    ResolvedPos, Schema, Slice,
};

use crate::{Extension, SchemaAdditions};

/// The lists extension. Adds three node types and binds `Mod-Shift-8` /
/// `Mod-Shift-7` to bullet- and ordered-list wrapping, and `Shift-Tab` to
/// [`lift_list_item`].
pub struct Lists;

fn ordered_list_attrs(el: &HtmlElement) -> Option<Attrs> {
    let mut a = Attrs::new();
    let start = el.attr("start").and_then(|s| s.parse::<u64>().ok()).unwrap_or(1);
    a.insert("start".to_string(), AttrValue::from(start));
    Some(a)
}

impl Extension for Lists {
    fn name(&self) -> &str {
        "lists"
    }

    fn schema_additions(&self) -> SchemaAdditions {
        let mut ordered_attrs = HashMap::new();
        ordered_attrs.insert(
            "start".to_string(),
            AttrSpec {
                default: Some(AttrValue::from(1u64)),
            },
        );
        SchemaAdditions {
            nodes: vec![
                (
                    "list_item".to_string(),
                    NodeSpec {
                        content: Some("block+".into()),
                        // NOT in the `block` group so a list_item cannot
                        // appear directly inside `doc` — only inside a
                        // bullet_list/ordered_list, which references it
                        // by name.
                        to_dom: Some(|_| DomSpec::element("li")),
                        parse_dom: vec![ParseRule::tag("li")],
                        ..Default::default()
                    },
                ),
                (
                    "bullet_list".to_string(),
                    NodeSpec {
                        content: Some("list_item+".into()),
                        group: Some("block".into()),
                        to_dom: Some(|_| DomSpec::element("ul")),
                        parse_dom: vec![ParseRule::tag("ul")],
                        ..Default::default()
                    },
                ),
                (
                    "ordered_list".to_string(),
                    NodeSpec {
                        content: Some("list_item+".into()),
                        group: Some("block".into()),
                        attrs: ordered_attrs,
                        to_dom: Some(|n| {
                            let start = n.attrs().get("start").and_then(|v| v.as_u64()).unwrap_or(1);
                            let mut spec = DomSpec::element("ol");
                            if start != 1 {
                                spec = spec.attr("start", start.to_string());
                            }
                            spec
                        }),
                        parse_dom: vec![ParseRule::with_attrs("ol", ordered_list_attrs)],
                        ..Default::default()
                    },
                ),
            ],
            ..Default::default()
        }
    }

    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        vec![
            ("Mod-Shift-8".to_string(), wrap_in_bullet_list()),
            ("Mod-Shift-7".to_string(), wrap_in_ordered_list()),
            ("Shift-Tab".to_string(), lift_list_item()),
        ]
    }
}

fn wrap_in_list(list_name: &'static str) -> Command {
    Box::new(move |state, dispatch| {
        let schema = state.schema();
        if schema.node_type(list_name).is_none() || schema.node_type("list_item").is_none() {
            return false;
        }
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        if rp.depth() == 0 {
            return false;
        }
        let block = rp.node(1).clone();
        if !block.node_type().is_block() {
            return false;
        }
        // Already inside a list_item? Then this command doesn't apply
        // (use lift/sink to navigate the structure instead).
        if rp.depth() >= 2 && rp.node(rp.depth() - 1).node_type().name() == "list_item" {
            return false;
        }
        let start = rp.before(1);
        let end = rp.after(1);
        let li = match schema.create_node("list_item", Attrs::new(), vec![block.clone()], vec![]) {
            Ok(n) => n,
            Err(_) => return false,
        };
        let list = match schema.create_node(list_name, Attrs::new(), vec![li], vec![]) {
            Ok(n) => n,
            Err(_) => return false,
        };
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let slice = Slice::new(Fragment::from_node(list), 0, 0);
            if tx
                .transform()
                .replace(start, end, slice, state.schema())
                .is_ok()
            {
                d(tx);
            }
        }
        true
    })
}

/// Wrap the enclosing block in a `bullet_list > list_item`.
pub fn wrap_in_bullet_list() -> Command {
    wrap_in_list("bullet_list")
}

/// Wrap the enclosing block in an `ordered_list > list_item`.
pub fn wrap_in_ordered_list() -> Command {
    wrap_in_list("ordered_list")
}

/// Lift the enclosing list_item out of its list. If the list has only one
/// list_item, the list is replaced by the item's content; otherwise the
/// item's content is lifted out and the list keeps its remaining siblings.
///
/// v0.1 covers the common case: a single-item list. Multi-item lifting
/// is deferred to v0.2 (it needs `ReplaceAroundStep`-driven surgery).
pub fn lift_list_item() -> Command {
    Box::new(|state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        // Walk up looking for a list_item ancestor whose parent is a list.
        let mut li_depth = None;
        for d in (1..=rp.depth()).rev() {
            if rp.node(d).node_type().name() == "list_item" {
                li_depth = Some(d);
                break;
            }
        }
        let Some(li_depth) = li_depth else {
            return false;
        };
        if li_depth == 0 {
            return false;
        }
        let list = rp.node(li_depth - 1);
        let list_name = list.node_type().name();
        if list_name != "bullet_list" && list_name != "ordered_list" {
            return false;
        }
        if list.child_count() != 1 {
            return false; // v0.1: only single-item lists lift cleanly
        }
        let start = rp.before(li_depth - 1);
        let end = rp.after(li_depth - 1);
        let item_content = rp.node(li_depth).content().clone();
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let slice = Slice::new(item_content, 0, 0);
            if tx
                .transform()
                .replace(start, end, slice, state.schema())
                .is_ok()
            {
                d(tx);
            }
        }
        true
    })
}
