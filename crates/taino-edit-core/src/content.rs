//! Content expressions: a minimal regex-like grammar describing which child
//! node types a parent may contain, compiled to a deterministic automaton.
//!
//! Supported grammar (v0.1): node/group names, sequence (juxtaposition),
//! `|` choice, parentheses, and the `+`, `*`, `?` quantifiers — e.g.
//! `"paragraph+"`, `"(text | image)*"`, `"heading paragraph+"`. Counted
//! ranges (`{n,m}`) are intentionally deferred (see ROADMAP).
//!
//! The construction (Thompson NFA then subset construction to a DFA) follows
//! ProseMirror's `ContentMatch` so behaviour matches the reference editor.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::error::SchemaError;

/// One transition out of an automaton state: a node type id and the target
/// state index.
#[derive(Debug, Clone)]
struct Transition {
    type_id: usize,
    to: usize,
}

/// A compiled content-match automaton, shared by every node type that uses
/// the equivalent expression instance.
#[derive(Debug)]
struct Automaton {
    states: Vec<State>,
}

#[derive(Debug, Default)]
struct State {
    valid_end: bool,
    next: Vec<Transition>,
}

/// A position within a node type's content automaton.
///
/// A [`ContentMatch`] is cheap to clone (it is an [`Arc`] handle plus a state
/// index) and is the unit returned by incremental matching:
/// [`match_type`](ContentMatch::match_type) and
/// [`match_types`](ContentMatch::match_types) advance the position, while
/// [`valid_end`](ContentMatch::valid_end) reports whether stopping here
/// satisfies the expression.
#[derive(Debug, Clone)]
pub struct ContentMatch {
    automaton: Arc<Automaton>,
    state: usize,
}

impl ContentMatch {
    /// Whether ending the content here satisfies the expression.
    pub fn valid_end(&self) -> bool {
        self.automaton.states[self.state].valid_end
    }

    /// Advance the match by a single child of type `type_id`, or `None` if no
    /// such child is allowed in this position.
    pub fn match_type(&self, type_id: usize) -> Option<ContentMatch> {
        let st = &self.automaton.states[self.state];
        for t in &st.next {
            if t.type_id == type_id {
                return Some(ContentMatch {
                    automaton: Arc::clone(&self.automaton),
                    state: t.to,
                });
            }
        }
        None
    }

    /// Advance the match across an ordered sequence of child type ids,
    /// returning the resulting position or `None` if the sequence is invalid.
    pub fn match_types<I: IntoIterator<Item = usize>>(&self, types: I) -> Option<ContentMatch> {
        let mut cur = self.clone();
        for ty in types {
            cur = cur.match_type(ty)?;
        }
        Some(cur)
    }

    /// Whether the given ordered child types form valid, complete content.
    pub fn matches_complete<I: IntoIterator<Item = usize>>(&self, types: I) -> bool {
        match self.match_types(types) {
            Some(end) => end.valid_end(),
            None => false,
        }
    }

    /// Whether two content automata share an acceptable first child type —
    /// the condition under which two nodes may be joined (mirrors
    /// ProseMirror's `ContentMatch.compatible`).
    pub fn compatible(&self, other: &ContentMatch) -> bool {
        let a = &self.automaton.states[self.state].next;
        let b = &other.automaton.states[other.state].next;
        a.iter().any(|x| b.iter().any(|y| x.type_id == y.type_id))
    }
}

// ---- expression AST -------------------------------------------------------

enum Expr {
    Choice(Vec<Expr>),
    Seq(Vec<Expr>),
    Plus(Box<Expr>),
    Star(Box<Expr>),
    Opt(Box<Expr>),
    /// The set of node-type ids this token may match (a plain name → one id,
    /// a group name → the group's ids).
    Match(Vec<usize>),
    Empty,
}

// ---- tokenizer ------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Tok {
    Name(String),
    Pipe,
    Open,
    Close,
    Plus,
    Star,
    Opt,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '|' => {
                chars.next();
                toks.push(Tok::Pipe);
            }
            '(' => {
                chars.next();
                toks.push(Tok::Open);
            }
            ')' => {
                chars.next();
                toks.push(Tok::Close);
            }
            '+' => {
                chars.next();
                toks.push(Tok::Plus);
            }
            '*' => {
                chars.next();
                toks.push(Tok::Star);
            }
            '?' => {
                chars.next();
                toks.push(Tok::Opt);
            }
            c if c.is_alphanumeric() || c == '_' || c == '-' => {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '-' {
                        name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                toks.push(Tok::Name(name));
            }
            other => return Err(format!("unexpected character `{other}`")),
        }
    }
    Ok(toks)
}

// ---- recursive-descent parser --------------------------------------------

struct Parser<'a, R: Fn(&str) -> Option<Vec<usize>>> {
    toks: Vec<Tok>,
    pos: usize,
    resolve: &'a R,
    bad_ref: Option<String>,
}

impl<'a, R: Fn(&str) -> Option<Vec<usize>>> Parser<'a, R> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let mut opts = vec![self.parse_seq()?];
        while self.eat(&Tok::Pipe) {
            opts.push(self.parse_seq()?);
        }
        Ok(if opts.len() == 1 {
            opts.pop().unwrap()
        } else {
            Expr::Choice(opts)
        })
    }

    fn parse_seq(&mut self) -> Result<Expr, String> {
        let mut parts = Vec::new();
        while self.at_seq_start() {
            parts.push(self.parse_postfix()?);
        }
        Ok(match parts.len() {
            0 => Expr::Empty,
            1 => parts.pop().unwrap(),
            _ => Expr::Seq(parts),
        })
    }

    fn at_seq_start(&self) -> bool {
        matches!(self.peek(), Some(Tok::Name(_)) | Some(Tok::Open))
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_atom()?;
        loop {
            if self.eat(&Tok::Plus) {
                e = Expr::Plus(Box::new(e));
            } else if self.eat(&Tok::Star) {
                e = Expr::Star(Box::new(e));
            } else if self.eat(&Tok::Opt) {
                e = Expr::Opt(Box::new(e));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_atom(&mut self) -> Result<Expr, String> {
        if self.eat(&Tok::Open) {
            let e = self.parse_expr()?;
            if !self.eat(&Tok::Close) {
                return Err("expected `)`".into());
            }
            Ok(e)
        } else if let Some(Tok::Name(_)) = self.peek() {
            let Some(Tok::Name(name)) = self.toks.get(self.pos) else {
                unreachable!()
            };
            let name = name.clone();
            self.pos += 1;
            match (self.resolve)(&name) {
                Some(ids) => Ok(Expr::Match(ids)),
                None => {
                    self.bad_ref = Some(name);
                    Err("unknown reference".into())
                }
            }
        } else {
            Err("expected a name or `(`".into())
        }
    }
}

// ---- NFA construction (Thompson) -----------------------------------------

struct Edge {
    term: Option<usize>,
    to: Option<usize>,
}

struct Nfa {
    nodes: Vec<Vec<Edge>>,
}

impl Nfa {
    fn node(&mut self) -> usize {
        self.nodes.push(Vec::new());
        self.nodes.len() - 1
    }

    fn edge(&mut self, from: usize, to: Option<usize>, term: Option<usize>) -> (usize, usize) {
        self.nodes[from].push(Edge { term, to });
        (from, self.nodes[from].len() - 1)
    }

    fn connect(&mut self, edges: &[(usize, usize)], to: usize) {
        for &(n, i) in edges {
            self.nodes[n][i].to = Some(to);
        }
    }

    fn compile(&mut self, expr: &Expr, from: usize) -> Vec<(usize, usize)> {
        match expr {
            Expr::Choice(opts) => {
                let mut out = Vec::new();
                for o in opts {
                    out.extend(self.compile(o, from));
                }
                out
            }
            Expr::Seq(parts) => {
                let mut from = from;
                for (i, p) in parts.iter().enumerate() {
                    let next = self.compile(p, from);
                    if i == parts.len() - 1 {
                        return next;
                    }
                    let n = self.node();
                    self.connect(&next, n);
                    from = n;
                }
                Vec::new()
            }
            Expr::Star(inner) => {
                let loop_node = self.node();
                let e = self.edge(from, Some(loop_node), None);
                let _ = e;
                let body = self.compile(inner, loop_node);
                self.connect(&body, loop_node);
                vec![self.edge(loop_node, None, None)]
            }
            Expr::Plus(inner) => {
                let loop_node = self.node();
                let body = self.compile(inner, from);
                self.connect(&body, loop_node);
                let body2 = self.compile(inner, loop_node);
                self.connect(&body2, loop_node);
                vec![self.edge(loop_node, None, None)]
            }
            Expr::Opt(inner) => {
                let mut out = vec![self.edge(from, None, None)];
                out.extend(self.compile(inner, from));
                out
            }
            Expr::Match(ids) => {
                let mut out = Vec::new();
                for &id in ids {
                    out.push(self.edge(from, None, Some(id)));
                }
                out
            }
            Expr::Empty => vec![self.edge(from, None, None)],
        }
    }
}

fn null_from(nfa: &Nfa, start: usize) -> Vec<usize> {
    let mut result = Vec::new();
    fn scan(nfa: &Nfa, node: usize, result: &mut Vec<usize>) {
        let edges = &nfa.nodes[node];
        if edges.len() == 1 && edges[0].term.is_none() {
            if let Some(to) = edges[0].to {
                return scan(nfa, to, result);
            }
        }
        if !result.contains(&node) {
            result.push(node);
        }
        for e in edges {
            if e.term.is_none() {
                if let Some(to) = e.to {
                    if !result.contains(&to) {
                        scan(nfa, to, result);
                    }
                }
            }
        }
    }
    scan(nfa, start, &mut result);
    result.sort_unstable();
    result.dedup();
    result
}

fn build_dfa(nfa: &Nfa, accept: usize) -> (Vec<State>, usize) {
    let mut states: Vec<State> = Vec::new();
    let mut labeled: BTreeMap<Vec<usize>, usize> = BTreeMap::new();

    fn explore(
        nfa: &Nfa,
        accept: usize,
        set: Vec<usize>,
        states: &mut Vec<State>,
        labeled: &mut BTreeMap<Vec<usize>, usize>,
    ) -> usize {
        if let Some(&idx) = labeled.get(&set) {
            return idx;
        }
        let idx = states.len();
        states.push(State {
            valid_end: set.contains(&accept),
            next: Vec::new(),
        });
        labeled.insert(set.clone(), idx);

        // Group reachable NFA nodes by transition term.
        let mut grouped: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for &node in &set {
            for e in &nfa.nodes[node] {
                if let (Some(term), Some(to)) = (e.term, e.to) {
                    let closure = null_from(nfa, to);
                    let bucket = grouped.entry(term).or_default();
                    for n in closure {
                        if !bucket.contains(&n) {
                            bucket.push(n);
                        }
                    }
                }
            }
        }

        let mut transitions = Vec::new();
        for (term, mut target) in grouped {
            target.sort_unstable();
            target.dedup();
            let to = explore(nfa, accept, target, states, labeled);
            transitions.push(Transition { type_id: term, to });
        }
        states[idx].next = transitions;
        idx
    }

    let start_set = null_from(nfa, 0);
    let start = explore(nfa, accept, start_set, &mut states, &mut labeled);
    (states, start)
}

/// Parse and compile a content expression.
///
/// `resolve` maps a name to the node-type ids it stands for (one id for a
/// plain node name, several for a group name); it returns `None` for unknown
/// references. An empty/whitespace-only expression yields a match that is
/// immediately a valid end and accepts nothing.
pub(crate) fn compile_content<R>(
    in_type: &str,
    src: &str,
    resolve: &R,
) -> Result<ContentMatch, SchemaError>
where
    R: Fn(&str) -> Option<Vec<usize>>,
{
    let toks = tokenize(src).map_err(|message| SchemaError::BadContentExpression {
        in_type: in_type.to_string(),
        message,
    })?;
    let mut parser = Parser {
        toks,
        pos: 0,
        resolve,
        bad_ref: None,
    };
    let expr = parser.parse_expr().map_err(|message| {
        if let Some(reference) = parser.bad_ref.take() {
            SchemaError::UnknownContentRef {
                in_type: in_type.to_string(),
                reference,
            }
        } else {
            SchemaError::BadContentExpression {
                in_type: in_type.to_string(),
                message,
            }
        }
    })?;
    if parser.pos != parser.toks.len() {
        return Err(SchemaError::BadContentExpression {
            in_type: in_type.to_string(),
            message: "trailing tokens after expression".into(),
        });
    }

    let mut nfa = Nfa {
        nodes: vec![Vec::new()],
    };
    let dangling = nfa.compile(&expr, 0);
    let accept = nfa.node();
    nfa.connect(&dangling, accept);

    let (states, start) = build_dfa(&nfa, accept);
    Ok(ContentMatch {
        automaton: Arc::new(Automaton { states }),
        state: start,
    })
}
