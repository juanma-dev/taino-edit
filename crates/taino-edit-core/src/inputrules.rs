//! [`InputRules`] — regex-triggered transforms applied to the text just
//! before the caret (e.g. `"## "` → heading, `"> "` → blockquote,
//! `"(c)"` → `©`).
//!
//! `core` is headless: an adapter calls [`InputRules::apply`] after each text
//! input; if a rule's pattern matches the text ending at the caret it returns
//! a ready [`Transaction`].

use regex::{Captures, Regex};

use crate::fragment::Fragment;
use crate::pos::ResolvedPos;
use crate::slice::Slice;
use crate::state::{EditorState, Transaction};

/// `(state, captures, from, to)` → an optional transaction. `from..to` is the
/// matched range (document positions) ending at the caret.
type Handler = Box<dyn Fn(&EditorState, &Captures<'_>, usize, usize) -> Option<Transaction>>;

/// A single input rule: a regex plus what to do when it matches.
pub struct InputRule {
    regex: Regex,
    handler: Handler,
}

impl InputRule {
    /// Compile a rule from `pattern` (matched against the text before the
    /// caret) and a handler.
    pub fn new(
        pattern: &str,
        handler: impl Fn(&EditorState, &Captures<'_>, usize, usize) -> Option<Transaction> + 'static,
    ) -> Result<InputRule, regex::Error> {
        Ok(InputRule {
            regex: Regex::new(pattern)?,
            handler: Box::new(handler),
        })
    }
}

/// An ordered set of [`InputRule`]s.
#[derive(Default)]
pub struct InputRules {
    rules: Vec<InputRule>,
}

impl InputRules {
    /// Build from a list of rules (tried in order).
    pub fn new(rules: Vec<InputRule>) -> Self {
        InputRules { rules }
    }

    /// Number of rules.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether there are no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// The plain text of the current textblock from its start up to an empty
    /// caret, or `None` if the selection is not a caret inside a textblock.
    fn text_before_caret(state: &EditorState) -> Option<(String, usize)> {
        let sel = state.selection();
        if !sel.is_empty() {
            return None;
        }
        let pos = sel.from();
        let rp = ResolvedPos::resolve(state.doc(), pos).ok()?;
        if rp.depth() == 0 {
            return None;
        }
        let start = rp.start(rp.depth());
        let slice = state.doc().slice(start, pos).ok()?;
        let text: String = slice.content().iter().map(|n| n.text_content()).collect();
        Some((text, pos))
    }

    /// Try every rule against the text before the caret; return the first
    /// resulting transaction.
    pub fn apply(&self, state: &EditorState) -> Option<Transaction> {
        let (text, pos) = Self::text_before_caret(state)?;
        for rule in &self.rules {
            let Some(caps) = rule.regex.captures(&text) else {
                continue;
            };
            let m = caps.get(0)?;
            if m.end() != text.len() {
                continue; // must match right up to the caret
            }
            let trigger_chars = text[m.start()..m.end()].chars().count();
            let from = pos - trigger_chars;
            if let Some(tx) = (rule.handler)(state, &caps, from, pos) {
                return Some(tx);
            }
        }
        None
    }
}

/// The top-level (depth-1) block enclosing `pos` in `state`'s doc, as
/// `(before, after)` positions.
fn top_block_range(state: &EditorState, pos: usize) -> Option<(usize, usize)> {
    let rp = ResolvedPos::resolve(state.doc(), pos).ok()?;
    if rp.depth() == 0 {
        return None;
    }
    Some((rp.before(1), rp.after(1)))
}

/// Replace the matched text with a literal string (e.g. `"(c)"` → `"©"`).
pub fn text_replace_rule(pattern: &str, replacement: &str) -> Result<InputRule, regex::Error> {
    let replacement = replacement.to_string();
    InputRule::new(pattern, move |state, _caps, from, to| {
        let mut tx = state.tr();
        let node = state.schema().text(&replacement, vec![]).ok()?;
        tx.transform().delete(from, to, state.schema()).ok()?;
        tx.transform()
            .insert(
                from,
                Slice::new(Fragment::from_node(node), 0, 0),
                state.schema(),
            )
            .ok()?;
        Some(tx)
    })
}

/// Drop the trigger text and retype the enclosing block (e.g. `"## "` →
/// heading). `attrs_from` derives the new block's attributes from the match.
pub fn textblock_type_rule(
    pattern: &str,
    node: &str,
    attrs_from: fn(&Captures<'_>) -> crate::attrs::Attrs,
) -> Result<InputRule, regex::Error> {
    let node = node.to_string();
    InputRule::new(pattern, move |state, caps, from, to| {
        let attrs = attrs_from(caps);
        let mut tx = state.tr();
        tx.transform().delete(from, to, state.schema()).ok()?;
        let after = tx.transform().doc().clone();
        let rp = ResolvedPos::resolve(&after, from).ok()?;
        if rp.depth() == 0 {
            return None;
        }
        let block = rp.node(1).clone();
        let (start, end) = (rp.before(1), rp.after(1));
        let new_block = state
            .schema()
            .node(
                &node,
                attrs,
                block.content().children().to_vec(),
                block.marks().to_vec(),
            )
            .ok()?;
        tx.transform()
            .replace(
                start,
                end,
                Slice::new(Fragment::from_node(new_block), 0, 0),
                state.schema(),
            )
            .ok()?;
        Some(tx)
    })
}

/// Drop the trigger text and wrap the enclosing block (e.g. `"> "` →
/// blockquote).
pub fn wrapping_rule(
    pattern: &str,
    node: &str,
    attrs: crate::attrs::Attrs,
) -> Result<InputRule, regex::Error> {
    let node = node.to_string();
    InputRule::new(pattern, move |state, _caps, from, to| {
        let mut tx = state.tr();
        tx.transform().delete(from, to, state.schema()).ok()?;
        let after = tx.transform().doc().clone();
        let mut probe = EditorState::new(after, state.schema().clone());
        {
            let mut t = probe.tr();
            t.set_selection(crate::selection::Selection::caret(from));
            probe = probe.apply(t);
        }
        let (start, end) = top_block_range(&probe, from)?;
        let wrapper = state
            .schema()
            .create_node(&node, attrs.clone(), vec![], vec![])
            .ok()?;
        let step = crate::step::ReplaceAroundStep::new(
            start,
            end,
            start,
            end,
            Slice::new(Fragment::from_node(wrapper), 0, 0),
            1,
        );
        tx.transform().step(Box::new(step), state.schema()).ok()?;
        Some(tx)
    })
}
