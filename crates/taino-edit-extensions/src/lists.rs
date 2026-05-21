//! Lists — `bullet_list`, `ordered_list` and `list_item` nodes plus the
//! full command vocabulary expected of a list-aware editor.
//!
//! v0.1 shipped only `wrap_in_*` and a single-item `lift_list_item`. v0.2
//! adds the rest of the canonical surface:
//!
//! * [`split_list_item`] — smart Enter inside a list item: splits the
//!   current textblock AND the enclosing list_item so a new bullet
//!   appears below the caret.
//! * [`sink_list_item`] — Tab to indent: the current list_item becomes a
//!   nested list inside its previous sibling.
//! * [`lift_list_item`] — generalised to handle multi-item lists (it
//!   splits the surviving items into before-list / after-list around
//!   the lifted blocks); the single-item case still works the same.
//! * [`smart_enter_in_list`] — convenience: lift if the caret sits in an
//!   empty list_item, otherwise split. The `Lists` extension binds it
//!   on `Enter` (after the base keymap's `split_block`).

use std::collections::HashMap;

use taino_edit_core::{
    chain, AttrSpec, AttrValue, Attrs, Command, DomSpec, Fragment, HtmlElement, NodeSpec,
    ParseRule, ResolvedPos, Schema, Selection, Slice,
};

use crate::{Extension, SchemaAdditions};

/// The lists extension. Adds three node types and binds `Mod-Shift-8` /
/// `Mod-Shift-7` to bullet- and ordered-list wrapping, and `Shift-Tab` to
/// [`lift_list_item`].
pub struct Lists;

fn ordered_list_attrs(el: &HtmlElement) -> Option<Attrs> {
    let mut a = Attrs::new();
    let start = el
        .attr("start")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
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
                            let start =
                                n.attrs().get("start").and_then(|v| v.as_u64()).unwrap_or(1);
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
            ("Tab".to_string(), sink_list_item()),
            ("Shift-Tab".to_string(), lift_list_item()),
            // Smart Enter must outrank the base keymap's `split_block`.
            // `chain` short-circuits: if the caret isn't inside a list
            // item, `smart_enter_in_list` reports false and the next
            // binding (base keymap's Enter) wins.
            (
                "Enter".to_string(),
                chain(vec![
                    smart_enter_in_list(),
                    Box::new(taino_edit_core::split_block),
                ]),
            ),
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

/// Find the closest list_item ancestor (and its enclosing list) above
/// `rp`. Returns `(li_depth, list_name)`; `None` if the caret isn't
/// inside a list_item or the parent at `li_depth - 1` isn't a known list
/// type.
fn nearest_list_item(rp: &ResolvedPos) -> Option<(usize, String)> {
    for d in (1..=rp.depth()).rev() {
        if rp.node(d).node_type().name() == "list_item" && d >= 1 {
            let parent_name = rp.node(d - 1).node_type().name();
            if parent_name == "bullet_list" || parent_name == "ordered_list" {
                return Some((d, parent_name.to_string()));
            }
        }
    }
    None
}

/// Lift the enclosing list_item out of its list. For a single-item list
/// the list disappears entirely (the item's blocks become siblings of
/// the list's old position). For a multi-item list the current item is
/// removed from the list and its blocks are inserted at the list's
/// position — the remaining items stay in a smaller list above the
/// lifted blocks (canonical "outdent the last bullet" behaviour).
pub fn lift_list_item() -> Command {
    Box::new(|state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((li_depth, _)) = nearest_list_item(&rp) else {
            return false;
        };
        let list = rp.node(li_depth - 1);
        let item_idx = rp.index(li_depth - 1);
        let item = rp.node(li_depth);
        let list_start = rp.before(li_depth - 1);
        let list_end = rp.after(li_depth - 1);

        // The lifted-out blocks (the list_item's content) become siblings
        // of the position the list previously occupied.
        let lifted: Vec<taino_edit_core::Node> = item.content().children().to_vec();

        // Build the replacement at the list's position: any siblings
        // before the current item stay in a shrunken list, then the
        // lifted blocks, then any siblings after stay in another list.
        let before_items: Vec<taino_edit_core::Node> =
            list.content().children()[..item_idx].to_vec();
        let after_items: Vec<taino_edit_core::Node> =
            list.content().children()[item_idx + 1..].to_vec();

        let mut replacement: Vec<taino_edit_core::Node> = Vec::new();
        if !before_items.is_empty() {
            let Ok(n) = state.schema().create_node(
                list.node_type().name(),
                list.attrs().clone(),
                before_items,
                list.marks().to_vec(),
            ) else {
                return false;
            };
            replacement.push(n);
        }
        replacement.extend(lifted);
        if !after_items.is_empty() {
            let Ok(n) = state.schema().create_node(
                list.node_type().name(),
                list.attrs().clone(),
                after_items,
                list.marks().to_vec(),
            ) else {
                return false;
            };
            replacement.push(n);
        }
        if replacement.is_empty() {
            return false;
        }
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let slice = Slice::new(Fragment::from_nodes(replacement), 0, 0);
            if tx
                .transform()
                .replace(list_start, list_end, slice, state.schema())
                .is_ok()
            {
                d(tx);
            }
        }
        true
    })
}

/// Smart Enter inside a list item. If the caret sits in an empty
/// textblock that is the only child of its list_item, lift the item
/// (exit the list); otherwise split the list_item and its textblock so
/// a fresh bullet appears below the caret. Returns `false` when the
/// caret isn't inside a list_item — letting the regular `split_block`
/// handle Enter elsewhere.
pub fn smart_enter_in_list() -> Command {
    Box::new(|state, dispatch| {
        let sel = state.selection();
        if !sel.is_empty() {
            return false;
        }
        let pos = sel.from();
        let Ok(rp) = ResolvedPos::resolve(state.doc(), pos) else {
            return false;
        };
        let Some((li_depth, _)) = nearest_list_item(&rp) else {
            return false;
        };
        let textblock_depth = rp.depth();
        if textblock_depth < li_depth {
            return false;
        }
        let textblock = rp.parent();
        let li_item = rp.node(li_depth);
        let textblock_is_empty = textblock.content().size() == 0;
        let li_has_one_block = li_item.child_count() == 1;
        if textblock_is_empty && li_has_one_block {
            // Empty bullet → exit the list via lift.
            return lift_list_item()(state, dispatch);
        }
        // Otherwise: split paragraph + list_item.
        let levels = textblock_depth - li_depth + 1;
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            if tx
                .transform()
                .split_at_depth(pos, levels, state.schema())
                .is_ok()
            {
                // Place the caret at the start of the second list_item's
                // first textblock. Each level adds 2 (close + open).
                tx.set_selection(Selection::caret(pos + 2 * levels));
                d(tx);
            }
        }
        true
    })
}

/// Public alias of [`smart_enter_in_list`] under the canonical PM name —
/// callers thinking "split list item" find what they expect.
pub fn split_list_item() -> Command {
    smart_enter_in_list()
}

/// Indent the current list_item: move it inside the previous sibling
/// list_item as a child of a new (same-kind) list. No-op when there is
/// no previous sibling.
pub fn sink_list_item() -> Command {
    Box::new(|state, dispatch| {
        let Ok(rp) = ResolvedPos::resolve(state.doc(), state.selection().from()) else {
            return false;
        };
        let Some((li_depth, list_name)) = nearest_list_item(&rp) else {
            return false;
        };
        let idx = rp.index(li_depth - 1);
        if idx == 0 {
            return false;
        }
        let list = rp.node(li_depth - 1);
        let prev_li = list.child(idx - 1).clone();
        let cur_li = list.child(idx).clone();

        // Append a new list (same kind as the outer one) containing
        // cur_li to prev_li's content.
        let Ok(nested_list) = state.schema().create_node(
            &list_name,
            list.attrs().clone(),
            vec![cur_li],
            list.marks().to_vec(),
        ) else {
            return false;
        };
        let mut new_prev_kids = prev_li.content().children().to_vec();
        new_prev_kids.push(nested_list);
        let Ok(new_prev_li) = state.schema().create_node(
            "list_item",
            prev_li.attrs().clone(),
            new_prev_kids,
            prev_li.marks().to_vec(),
        ) else {
            return false;
        };

        // Rebuild the parent list without `cur_li` and with prev_li
        // replaced by new_prev_li.
        let mut new_children = list.content().children().to_vec();
        new_children[idx - 1] = new_prev_li;
        new_children.remove(idx);
        let Ok(new_list_node) = state.schema().create_node(
            &list_name,
            list.attrs().clone(),
            new_children,
            list.marks().to_vec(),
        ) else {
            return false;
        };
        let start = rp.before(li_depth - 1);
        let end = rp.after(li_depth - 1);
        if let Some(d) = dispatch {
            let mut tx = state.tr();
            let slice = Slice::new(Fragment::from_node(new_list_node), 0, 0);
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
