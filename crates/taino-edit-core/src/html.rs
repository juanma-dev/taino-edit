//! HTML serialization (doc → string) and a strict, dependency-free HTML
//! parser (string → doc).
//!
//! Design and safety notes:
//!
//! - **No third-party HTML engine.** A small hand-written tokenizer keeps
//!   `core` lean and `#![deny(unsafe_code)]`, and minimizes supply-chain and
//!   parsing-attack surface. It deliberately understands only the subset an
//!   editor needs; it is not a full HTML5 conformance parser.
//! - **Output is always escaped.** Text and attribute values are HTML-escaped;
//!   the serializer never emits raw markup, so a document cannot inject
//!   `<script>` or break out of an attribute.
//! - **Input is schema-gated.** Only tags for which the schema declares a
//!   [`ParseRule`] become nodes/marks; unknown elements are unwrapped (their
//!   children are kept) and the assembled tree is validated against the
//!   schema, so structurally invalid HTML is rejected rather than trusted.
//! - **Hostile input is bounded.** Nesting depth is capped
//!   ([`MAX_DEPTH`]); pathological input yields [`DocError::HtmlParse`]
//!   instead of unbounded recursion or memory growth.

use std::collections::BTreeMap;

use crate::attrs::Attrs;
use crate::error::DocError;
use crate::mark::Mark;
use crate::node::Node;
use crate::schema::Schema;

/// Maximum element nesting depth accepted by [`Schema::parse_html`]. Input
/// that nests deeper is rejected with [`DocError::HtmlParse`].
pub const MAX_DEPTH: usize = 100;

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/// How to render a node or mark as an HTML element.
///
/// Build with [`DomSpec::element`] (a container with a content hole) or
/// [`DomSpec::void`] (a childless element such as `<img>`), then chain
/// [`DomSpec::attr`].
#[derive(Debug, Clone)]
pub struct DomSpec {
    tag: String,
    attrs: Vec<(String, String)>,
    content_hole: bool,
}

impl DomSpec {
    /// A container element whose children are serialized inside it.
    pub fn element(tag: &str) -> Self {
        DomSpec {
            tag: tag.to_string(),
            attrs: Vec::new(),
            content_hole: true,
        }
    }

    /// A childless element (e.g. `<img>`, `<hr>`).
    pub fn void(tag: &str) -> Self {
        DomSpec {
            tag: tag.to_string(),
            attrs: Vec::new(),
            content_hole: false,
        }
    }

    /// Add an attribute. Order is preserved in the output.
    pub fn attr(mut self, name: &str, value: impl Into<String>) -> Self {
        self.attrs.push((name.to_string(), value.into()));
        self
    }
}

/// A parsed HTML element exposed to [`ParseRule`] attribute extractors.
#[derive(Debug, Clone)]
pub struct HtmlElement {
    /// Lowercased tag name.
    pub tag: String,
    attrs: BTreeMap<String, String>,
}

impl HtmlElement {
    /// The value of attribute `name`, if present.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }
}

/// A rule mapping an HTML tag back to the node or mark type whose spec
/// declares it.
#[derive(Debug, Clone)]
pub struct ParseRule {
    /// Tag name to match (compared case-insensitively).
    pub tag: String,
    /// Derive attributes from the matched element. `None` means no
    /// attributes; a closure returning `None` declines the match so another
    /// rule may apply.
    pub get_attrs: Option<fn(&HtmlElement) -> Option<Attrs>>,
}

impl ParseRule {
    /// A rule matching `tag` with no attributes.
    pub fn tag(tag: &str) -> Self {
        ParseRule {
            tag: tag.to_string(),
            get_attrs: None,
        }
    }

    /// A rule matching `tag` that derives attributes via `f`.
    pub fn with_attrs(tag: &str, f: fn(&HtmlElement) -> Option<Attrs>) -> Self {
        ParseRule {
            tag: tag.to_string(),
            get_attrs: Some(f),
        }
    }
}

// ---- serialization --------------------------------------------------------

fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn open_tag(spec: &DomSpec) -> String {
    let mut s = String::new();
    s.push('<');
    s.push_str(&spec.tag);
    for (k, v) in &spec.attrs {
        s.push(' ');
        s.push_str(k);
        s.push_str("=\"");
        s.push_str(&escape_attr(v));
        s.push('"');
    }
    s
}

impl Node {
    /// Serialize this node (and its subtree) to an HTML string.
    ///
    /// Text and attribute values are HTML-escaped. A node type whose spec has
    /// no `to_dom` is transparent — only its content is emitted (the usual
    /// case for the document node), which is why a serialized document is a
    /// run of its block children with no wrapper.
    pub fn to_html(&self) -> String {
        if let Some(text) = self.text() {
            let mut s = escape_text(text);
            for mark in self.marks() {
                if let Some(f) = mark.mark_type().spec().to_dom {
                    let spec = f(mark);
                    s = format!("{}>{}</{}>", open_tag(&spec), s, spec.tag);
                }
            }
            return s;
        }

        let children: String = self.content().iter().map(Node::to_html).collect();
        match self.node_type().spec().to_dom {
            None => children,
            Some(f) => {
                let spec = f(self);
                if spec.content_hole {
                    format!("{}>{}</{}>", open_tag(&spec), children, spec.tag)
                } else {
                    format!("{}/>", open_tag(&spec))
                }
            }
        }
    }
}

// ---- tokenizer ------------------------------------------------------------

#[derive(Debug)]
enum Token {
    Open {
        tag: String,
        attrs: BTreeMap<String, String>,
        self_closing: bool,
    },
    Close(String),
    Text(String),
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '&' {
            if let Some(semi) = bytes[i + 1..].iter().position(|&c| c == ';') {
                let ent: String = bytes[i + 1..i + 1 + semi].iter().collect();
                let decoded = match ent.as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                        u32::from_str_radix(&ent[2..], 16)
                            .ok()
                            .and_then(char::from_u32)
                    }
                    _ if ent.starts_with('#') => {
                        ent[1..].parse::<u32>().ok().and_then(char::from_u32)
                    }
                    _ => None,
                };
                if let Some(c) = decoded {
                    out.push(c);
                    i += semi + 2;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn tokenize(html: &str) -> Vec<Token> {
    let chars: Vec<char> = html.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < n {
        if chars[i] == '<' {
            // Comment / CDATA / doctype / processing instruction → skipped.
            if chars[i + 1..].starts_with(&['!', '-', '-']) {
                if let Some(end) = find_subseq(&chars, i + 4, &['-', '-', '>']) {
                    i = end + 3;
                } else {
                    i = n;
                }
                continue;
            }
            if chars.get(i + 1) == Some(&'!') || chars.get(i + 1) == Some(&'?') {
                i = chars[i..]
                    .iter()
                    .position(|&c| c == '>')
                    .map_or(n, |p| i + p + 1);
                continue;
            }
            if chars.get(i + 1) == Some(&'/') {
                let mut j = i + 2;
                let mut name = String::new();
                while j < n && chars[j] != '>' {
                    name.push(chars[j]);
                    j += 1;
                }
                tokens.push(Token::Close(name.trim().to_ascii_lowercase()));
                i = j + 1;
                continue;
            }
            // Opening tag.
            let mut j = i + 1;
            let mut tag = String::new();
            while j < n && !chars[j].is_whitespace() && chars[j] != '>' && chars[j] != '/' {
                tag.push(chars[j]);
                j += 1;
            }
            let mut attrs = BTreeMap::new();
            let mut self_closing = false;
            loop {
                while j < n && chars[j].is_whitespace() {
                    j += 1;
                }
                if j >= n || chars[j] == '>' {
                    break;
                }
                if chars[j] == '/' {
                    self_closing = true;
                    j += 1;
                    continue;
                }
                let mut name = String::new();
                while j < n
                    && !chars[j].is_whitespace()
                    && chars[j] != '='
                    && chars[j] != '>'
                    && chars[j] != '/'
                {
                    name.push(chars[j]);
                    j += 1;
                }
                while j < n && chars[j].is_whitespace() {
                    j += 1;
                }
                let mut value = String::new();
                if j < n && chars[j] == '=' {
                    j += 1;
                    while j < n && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j < n && (chars[j] == '"' || chars[j] == '\'') {
                        let quote = chars[j];
                        j += 1;
                        while j < n && chars[j] != quote {
                            value.push(chars[j]);
                            j += 1;
                        }
                        j += 1;
                    } else {
                        while j < n
                            && !chars[j].is_whitespace()
                            && chars[j] != '>'
                            && chars[j] != '/'
                        {
                            value.push(chars[j]);
                            j += 1;
                        }
                    }
                }
                if !name.is_empty() {
                    attrs.insert(name.to_ascii_lowercase(), decode_entities(&value));
                }
            }
            let tag = tag.to_ascii_lowercase();
            if VOID_TAGS.contains(&tag.as_str()) {
                self_closing = true;
            }
            tokens.push(Token::Open {
                tag,
                attrs,
                self_closing,
            });
            i = j + 1;
        } else {
            let mut text = String::new();
            while i < n && chars[i] != '<' {
                text.push(chars[i]);
                i += 1;
            }
            tokens.push(Token::Text(decode_entities(&text)));
        }
    }
    tokens
}

fn find_subseq(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    if from > chars.len() {
        return None;
    }
    chars[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

// ---- tree ----------------------------------------------------------------

#[derive(Debug)]
enum DomTree {
    Element {
        tag: String,
        attrs: BTreeMap<String, String>,
        children: Vec<DomTree>,
    },
    Text(String),
}

struct Frame {
    tag: String,
    attrs: BTreeMap<String, String>,
    children: Vec<DomTree>,
}

fn build_tree(tokens: Vec<Token>) -> Result<Vec<DomTree>, DocError> {
    let mut root: Vec<DomTree> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();

    macro_rules! push_child {
        ($node:expr) => {
            match stack.last_mut() {
                Some(f) => f.children.push($node),
                None => root.push($node),
            }
        };
    }

    for tok in tokens {
        match tok {
            Token::Text(t) => push_child!(DomTree::Text(t)),
            Token::Open {
                tag,
                attrs,
                self_closing,
            } => {
                if self_closing {
                    push_child!(DomTree::Element {
                        tag,
                        attrs,
                        children: Vec::new()
                    });
                } else {
                    if stack.len() >= MAX_DEPTH {
                        return Err(DocError::HtmlParse(format!(
                            "element nesting exceeds {MAX_DEPTH}"
                        )));
                    }
                    stack.push(Frame {
                        tag,
                        attrs,
                        children: Vec::new(),
                    });
                }
            }
            Token::Close(tag) => {
                if let Some(depth) = stack.iter().rposition(|f| f.tag == tag) {
                    // Auto-close any intervening unclosed elements.
                    while stack.len() > depth {
                        let f = stack.pop().unwrap();
                        let el = DomTree::Element {
                            tag: f.tag,
                            attrs: f.attrs,
                            children: f.children,
                        };
                        push_child!(el);
                    }
                }
                // A stray close with no matching open is ignored.
            }
        }
    }
    // Unwind anything left open.
    while let Some(f) = stack.pop() {
        let el = DomTree::Element {
            tag: f.tag,
            attrs: f.attrs,
            children: f.children,
        };
        push_child!(el);
    }
    Ok(root)
}

// ---- conversion to nodes -------------------------------------------------

fn is_ws_text(n: &Node) -> bool {
    n.text().is_some_and(|t| t.chars().all(char::is_whitespace))
}

impl Schema {
    fn fill_mark_attrs(&self, mark: &str, mut given: Attrs) -> Attrs {
        if let Some(mt) = self.mark_type(mark) {
            for (k, s) in &mt.spec().attrs {
                if !given.contains_key(k) {
                    if let Some(d) = &s.default {
                        given.insert(k.clone(), d.clone());
                    }
                }
            }
        }
        given
    }

    fn match_mark(&self, el: &HtmlElement) -> Option<(String, Attrs)> {
        for mt in self.mark_types() {
            for rule in &mt.spec().parse_dom {
                if rule.tag.eq_ignore_ascii_case(&el.tag) {
                    let attrs = match rule.get_attrs {
                        None => Some(Attrs::new()),
                        Some(f) => f(el),
                    };
                    if let Some(a) = attrs {
                        return Some((mt.name().to_string(), self.fill_mark_attrs(mt.name(), a)));
                    }
                }
            }
        }
        None
    }

    fn match_node(&self, el: &HtmlElement) -> Option<(String, Attrs)> {
        for nt in self.node_types() {
            for rule in &nt.spec().parse_dom {
                if rule.tag.eq_ignore_ascii_case(&el.tag) {
                    let attrs = match rule.get_attrs {
                        None => Some(Attrs::new()),
                        Some(f) => f(el),
                    };
                    if let Some(a) = attrs {
                        return Some((nt.name().to_string(), a));
                    }
                }
            }
        }
        None
    }

    fn convert(
        &self,
        trees: &[DomTree],
        marks: &[Mark],
        depth: usize,
    ) -> Result<Vec<Node>, DocError> {
        if depth > MAX_DEPTH {
            return Err(DocError::HtmlParse(format!(
                "element nesting exceeds {MAX_DEPTH}"
            )));
        }
        let mut out = Vec::new();
        for tree in trees {
            match tree {
                DomTree::Text(t) => {
                    if !t.is_empty() {
                        out.push(self.text(t, marks.to_vec())?);
                    }
                }
                DomTree::Element {
                    tag,
                    attrs,
                    children,
                } => {
                    let el = HtmlElement {
                        tag: tag.clone(),
                        attrs: attrs.clone(),
                    };
                    if let Some((mark_name, mark_attrs)) = self.match_mark(&el) {
                        let m = self.mark_type(&mark_name).unwrap().create(mark_attrs);
                        let new_marks = m.add_to_set(marks);
                        out.extend(self.convert(children, &new_marks, depth + 1)?);
                    } else if let Some((node_name, node_attrs)) = self.match_node(&el) {
                        // Marks are inline-scoped: a fresh element resets them.
                        let kids = self.convert(children, &[], depth + 1)?;
                        out.push(self.build_node(&node_name, node_attrs, kids)?);
                    } else {
                        // Unknown element: unwrap, keep its content.
                        out.extend(self.convert(children, marks, depth + 1)?);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Build a node, retrying once without whitespace-only text children if
    /// the first attempt violates the content expression (handles insignificant
    /// inter-tag whitespace without loosening strictness for real content).
    fn build_node(&self, name: &str, attrs: Attrs, kids: Vec<Node>) -> Result<Node, DocError> {
        match self.node(name, attrs.clone(), kids.clone(), vec![]) {
            Ok(n) => Ok(n),
            Err(DocError::InvalidContent { .. }) => {
                let filtered: Vec<Node> = kids.into_iter().filter(|n| !is_ws_text(n)).collect();
                self.node(name, attrs, filtered, vec![])
            }
            Err(e) => Err(e),
        }
    }

    /// Parse an HTML string into a document, strictly validated against this
    /// schema.
    ///
    /// Recognized tags (those a node/mark spec declares via [`ParseRule`])
    /// become nodes/marks; unknown elements are unwrapped. The result is
    /// wrapped in (or returned as) the schema's top node and validated, so
    /// content that cannot satisfy the schema yields
    /// [`DocError::InvalidContent`]. Overly deep input yields
    /// [`DocError::HtmlParse`].
    pub fn parse_html(&self, html: &str) -> Result<Node, DocError> {
        let trees = build_tree(tokenize(html))?;
        let children = self.convert(&trees, &[], 0)?;

        let top = self.top_node_type().name().to_string();
        if children.len() == 1 && children[0].node_type().name() == top {
            return Ok(children[0].clone());
        }
        self.build_node(&top, Attrs::new(), children)
    }
}
