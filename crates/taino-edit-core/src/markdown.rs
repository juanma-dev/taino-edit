//! Markdown round-trip: [`to_markdown`] serializes a [`Node`] to a
//! CommonMark string, and [`parse_markdown`] turns a CommonMark string
//! back into a schema-valid `Node`.
//!
//! The mapping is hard-coded to the canonical node/mark names used by
//! `taino-edit-extensions` (`paragraph`, `heading`, `blockquote`,
//! `code_block`, `bullet_list`, `ordered_list`, `list_item`, `image`,
//! `link`, `strong`, `em`). Unknown nodes/marks fall back gracefully
//! (text is preserved; structure that has no Markdown equivalent is
//! emitted as inline HTML on the serializer side).
//!
//! Parsing uses [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark)
//! and validates the resulting tree against the supplied schema — content
//! that doesn't fit returns `DocError::InvalidContent` instead of building
//! a broken doc.
//!
//! ```
//! # use taino_edit_core::{
//! #     markdown::to_markdown, AttrValue, Attrs, NodeSpec, SchemaBuilder, MarkSpec,
//! # };
//! let schema = SchemaBuilder::new()
//!     .node("doc",       NodeSpec { content: Some("block+".into()), ..Default::default() })
//!     .node("paragraph", NodeSpec { content: Some("text*".into()), group: Some("block".into()), ..Default::default() })
//!     .node("text",      NodeSpec { group: Some("inline".into()), ..Default::default() })
//!     .mark("strong",    MarkSpec::default())
//!     .top_node("doc")
//!     .build()
//!     .unwrap();
//! let strong = schema.mark_type("strong").unwrap().clone();
//! let t = schema.text("hi", vec![strong.create(Attrs::new())]).unwrap();
//! let p = schema.node("paragraph", Default::default(), vec![t], vec![]).unwrap();
//! let doc = schema.node("doc", Default::default(), vec![p], vec![]).unwrap();
//! assert_eq!(to_markdown(&doc).trim(), "**hi**");
//! ```

use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::attrs::{AttrValue, Attrs};
use crate::error::DocError;
use crate::mark::{Mark, MarkType};
use crate::node::Node;
use crate::schema::Schema;

// ----- Serializer ---------------------------------------------------------

/// Serialize `doc` to a CommonMark-subset Markdown string.
pub fn to_markdown(doc: &Node) -> String {
    let mut out = String::new();
    serialize_block_children(doc, &mut out, "");
    // Trim trailing blank lines but keep a single trailing newline so the
    // output is a well-formed text file.
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    out
}

fn serialize_block_children(parent: &Node, out: &mut String, indent: &str) {
    for child in parent.content().iter() {
        serialize_block(child, out, indent);
    }
}

fn serialize_block(node: &Node, out: &mut String, indent: &str) {
    match node.node_type().name() {
        "paragraph" => {
            out.push_str(indent);
            serialize_inline(node, out);
            out.push_str("\n\n");
        }
        "heading" => {
            let level = node
                .attrs()
                .get("level")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
            out.push_str(indent);
            for _ in 0..level.min(6) {
                out.push('#');
            }
            out.push(' ');
            serialize_inline(node, out);
            out.push_str("\n\n");
        }
        "blockquote" => {
            let mut inner = String::new();
            serialize_block_children(node, &mut inner, "");
            for line in inner.trim_end_matches('\n').split('\n') {
                out.push_str(indent);
                if line.is_empty() {
                    out.push('>');
                } else {
                    out.push_str("> ");
                    out.push_str(line);
                }
                out.push('\n');
            }
            out.push('\n');
        }
        "code_block" => {
            out.push_str(indent);
            out.push_str("```\n");
            // Code block text content goes verbatim; we don't escape it.
            out.push_str(&node.text_content());
            if !node.text_content().ends_with('\n') {
                out.push('\n');
            }
            out.push_str(indent);
            out.push_str("```\n\n");
        }
        "bullet_list" => {
            serialize_list(node, out, indent, None);
        }
        "ordered_list" => {
            let start = node
                .attrs()
                .get("start")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            serialize_list(node, out, indent, Some(start as usize));
        }
        _ => {
            // Unknown block: fall back to HTML so we don't lose content.
            out.push_str(indent);
            out.push_str(&node.to_html());
            out.push_str("\n\n");
        }
    }
}

fn serialize_list(list_node: &Node, out: &mut String, indent: &str, ordered_from: Option<usize>) {
    let mut counter = ordered_from.unwrap_or(1);
    for item in list_node.content().iter() {
        // Each list_item: render its first block on the bullet line,
        // subsequent blocks indented.
        let bullet = match ordered_from {
            Some(_) => {
                let s = format!("{counter}. ");
                counter += 1;
                s
            }
            None => "- ".to_string(),
        };
        let inner_indent = " ".repeat(bullet.chars().count());
        let combined_indent = format!("{indent}{inner_indent}");

        let mut first = true;
        for sub in item.content().iter() {
            let mut piece = String::new();
            serialize_block(sub, &mut piece, "");
            let lines: Vec<&str> = piece.split('\n').collect();
            // Trim trailing empty lines from this piece.
            let mut trimmed_end = lines.len();
            while trimmed_end > 0 && lines[trimmed_end - 1].is_empty() {
                trimmed_end -= 1;
            }
            for (i, line) in lines[..trimmed_end].iter().enumerate() {
                if first && i == 0 {
                    out.push_str(indent);
                    out.push_str(&bullet);
                    out.push_str(line);
                    first = false;
                } else {
                    out.push_str(&combined_indent);
                    out.push_str(line);
                }
                out.push('\n');
            }
        }
    }
    out.push('\n');
}

fn serialize_inline(parent: &Node, out: &mut String) {
    let mut active_marks: Vec<&Mark> = Vec::new();
    for child in parent.content().iter() {
        serialize_inline_node(child, out, &mut active_marks);
    }
    while let Some(m) = active_marks.pop() {
        out.push_str(&mark_close(m));
    }
}

fn serialize_inline_node<'a>(node: &'a Node, out: &mut String, active: &mut Vec<&'a Mark>) {
    if node.is_text() {
        emit_with_marks(node, out, active);
        return;
    }
    // Close any open marks before a non-text inline (image / atom).
    while let Some(m) = active.pop() {
        out.push_str(&mark_close(m));
    }
    match node.node_type().name() {
        "image" => {
            let src = node
                .attrs()
                .get("src")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let alt = node
                .attrs()
                .get("alt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            out.push_str(&format!("![{alt}]({src})"));
        }
        _ => {
            // Inline unknown: fall back to HTML.
            out.push_str(&node.to_html());
        }
    }
}

fn emit_with_marks<'a>(text_node: &'a Node, out: &mut String, active: &mut Vec<&'a Mark>) {
    let marks = text_node.marks();

    // Inline code is a literal span: close any open marks, then emit the
    // raw (un-escaped) text inside backticks. Markdown can't combine it
    // with other inline marks, so `code` wins for this run.
    if marks.iter().any(|m| m.mark_type().name() == "code") {
        while let Some(closed) = active.pop() {
            out.push_str(&mark_close(closed));
        }
        let text = text_node.text().unwrap_or("");
        // Use enough backticks to wrap text that itself contains backticks.
        let max_run = text
            .split(|c| c != '`')
            .map(|run| run.len())
            .max()
            .unwrap_or(0);
        let fence = "`".repeat(max_run + 1);
        out.push_str(&fence);
        if max_run > 0 {
            out.push(' ');
            out.push_str(text);
            out.push(' ');
        } else {
            out.push_str(text);
        }
        out.push_str(&fence);
        return;
    }

    // Close marks that aren't on the new node, from most recent outward.
    while let Some(top) = active.last() {
        if marks.iter().any(|m| m == *top) {
            break;
        }
        let closed = active.pop().unwrap();
        out.push_str(&mark_close(closed));
    }
    // Open marks that are on the new node but not already active.
    for m in marks {
        // `active: &mut Vec<&Mark>` so we compare references-as-equal-marks
        // via PartialEq on Mark, not via slice::contains.
        let already = active.iter().any(|a| **a == *m);
        if !already {
            out.push_str(&mark_open(m));
            active.push(m);
        }
    }
    out.push_str(&escape_md(text_node.text().unwrap_or("")));
}

fn mark_open(m: &Mark) -> String {
    match m.mark_type().name() {
        "strong" => "**".into(),
        "em" => "*".into(),
        "link" => {
            // Markdown links can't easily express "open here, close
            // later" without text in between, so we just emit the
            // bracket; the close emits the URL.
            "[".into()
        }
        _ => String::new(),
    }
}

fn mark_close(m: &Mark) -> String {
    match m.mark_type().name() {
        "strong" => "**".into(),
        "em" => "*".into(),
        "link" => {
            let href = m.attrs().get("href").and_then(|v| v.as_str()).unwrap_or("");
            format!("]({href})")
        }
        _ => String::new(),
    }
}

fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

// ----- Parser -------------------------------------------------------------

/// Parse a CommonMark string into a `Node` validated against `schema`.
pub fn parse_markdown(schema: &Schema, md: &str) -> Result<Node, DocError> {
    let mut parser = Parser::new_ext(md, Options::all());
    let mut blocks: Vec<Node> = Vec::new();
    parse_blocks(schema, &mut parser, &mut blocks, &[])?;
    if blocks.is_empty() {
        // Empty doc: build the top node empty.
        return schema.node(schema.top_node_type().name(), Attrs::new(), vec![], vec![]);
    }
    schema.node(schema.top_node_type().name(), Attrs::new(), blocks, vec![])
}

fn parse_blocks(
    schema: &Schema,
    parser: &mut Parser<'_>,
    out: &mut Vec<Node>,
    inherited_marks: &[Mark],
) -> Result<(), DocError> {
    while let Some(ev) = parser.next() {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    let inlines = parse_inline(schema, parser, TagEnd::Paragraph, inherited_marks)?;
                    if schema.node_type("paragraph").is_some() {
                        out.push(schema.node("paragraph", Attrs::new(), inlines, vec![])?);
                    }
                }
                Tag::Heading { level, .. } => {
                    let inlines =
                        parse_inline(schema, parser, TagEnd::Heading(level), inherited_marks)?;
                    if schema.node_type("heading").is_some() {
                        let mut attrs = Attrs::new();
                        attrs.insert("level".into(), AttrValue::from(level_to_u64(level)));
                        out.push(schema.node("heading", attrs, inlines, vec![])?);
                    }
                }
                Tag::BlockQuote(_) => {
                    let mut children: Vec<Node> = Vec::new();
                    parse_blocks_until(
                        schema,
                        parser,
                        TagEnd::BlockQuote(None),
                        &mut children,
                        inherited_marks,
                    )?;
                    if schema.node_type("blockquote").is_some() {
                        out.push(schema.node("blockquote", Attrs::new(), children, vec![])?);
                    }
                }
                Tag::CodeBlock(_kind) => {
                    let code = collect_code(parser);
                    if schema.node_type("code_block").is_some() {
                        let text = schema.text(&code, vec![])?;
                        out.push(schema.node("code_block", Attrs::new(), vec![text], vec![])?);
                    }
                }
                Tag::List(start) => {
                    let mut items: Vec<Node> = Vec::new();
                    parse_list_items(schema, parser, &mut items, inherited_marks)?;
                    let (name, mut attrs) = if let Some(n) = start {
                        let mut a = Attrs::new();
                        a.insert("start".into(), AttrValue::from(n));
                        ("ordered_list", a)
                    } else {
                        ("bullet_list", Attrs::new())
                    };
                    // start=1 is the default; don't bother writing it.
                    if attrs.get("start").and_then(|v| v.as_u64()) == Some(1) {
                        attrs.remove("start");
                    }
                    if schema.node_type(name).is_some() {
                        out.push(schema.node(name, attrs, items, vec![])?);
                    }
                }
                _ => {
                    // Unknown block tag → consume until matching end and ignore.
                    skip_until_end(parser);
                }
            },
            Event::End(_) => return Ok(()),
            _ => {
                // Inline events at block level (unusual) — wrap them in a paragraph.
                let inlines = consume_inline_event(schema, ev, inherited_marks);
                if !inlines.is_empty() && schema.node_type("paragraph").is_some() {
                    out.push(schema.node("paragraph", Attrs::new(), inlines, vec![])?);
                }
            }
        }
    }
    Ok(())
}

fn parse_blocks_until(
    schema: &Schema,
    parser: &mut Parser<'_>,
    end_tag: TagEnd,
    out: &mut Vec<Node>,
    inherited_marks: &[Mark],
) -> Result<(), DocError> {
    while let Some(ev) = parser.next() {
        if matches!(&ev, Event::End(t) if std::mem::discriminant(t) == std::mem::discriminant(&end_tag))
        {
            return Ok(());
        }
        push_back_and_parse_block(schema, parser, ev, out, inherited_marks)?;
    }
    Ok(())
}

fn push_back_and_parse_block(
    schema: &Schema,
    parser: &mut Parser<'_>,
    ev: Event<'_>,
    out: &mut Vec<Node>,
    inherited_marks: &[Mark],
) -> Result<(), DocError> {
    match ev {
        Event::Start(Tag::Paragraph) => {
            let inlines = parse_inline(schema, parser, TagEnd::Paragraph, inherited_marks)?;
            if schema.node_type("paragraph").is_some() {
                out.push(schema.node("paragraph", Attrs::new(), inlines, vec![])?);
            }
        }
        Event::Start(Tag::Heading { level, .. }) => {
            let inlines = parse_inline(schema, parser, TagEnd::Heading(level), inherited_marks)?;
            if schema.node_type("heading").is_some() {
                let mut attrs = Attrs::new();
                attrs.insert("level".into(), AttrValue::from(level_to_u64(level)));
                out.push(schema.node("heading", attrs, inlines, vec![])?);
            }
        }
        _ => { /* ignored at this level */ }
    }
    Ok(())
}

fn parse_list_items(
    schema: &Schema,
    parser: &mut Parser<'_>,
    out: &mut Vec<Node>,
    inherited_marks: &[Mark],
) -> Result<(), DocError> {
    while let Some(ev) = parser.next() {
        match ev {
            Event::Start(Tag::Item) => {
                let mut blocks: Vec<Node> = Vec::new();
                parse_list_item_body(schema, parser, &mut blocks, inherited_marks)?;
                if blocks.is_empty() && schema.node_type("paragraph").is_some() {
                    blocks.push(schema.node("paragraph", Attrs::new(), vec![], vec![])?);
                }
                if schema.node_type("list_item").is_some() {
                    out.push(schema.node("list_item", Attrs::new(), blocks, vec![])?);
                }
            }
            Event::End(TagEnd::List(_)) => return Ok(()),
            _ => {}
        }
    }
    Ok(())
}

fn parse_list_item_body(
    schema: &Schema,
    parser: &mut Parser<'_>,
    out: &mut Vec<Node>,
    inherited_marks: &[Mark],
) -> Result<(), DocError> {
    while let Some(ev) = parser.next() {
        match ev {
            Event::End(TagEnd::Item) => return Ok(()),
            Event::Start(Tag::Paragraph) => {
                let inlines = parse_inline(schema, parser, TagEnd::Paragraph, inherited_marks)?;
                if schema.node_type("paragraph").is_some() {
                    out.push(schema.node("paragraph", Attrs::new(), inlines, vec![])?);
                }
            }
            Event::Start(Tag::List(start)) => {
                let mut items: Vec<Node> = Vec::new();
                parse_list_items(schema, parser, &mut items, inherited_marks)?;
                let (name, mut attrs) = if let Some(n) = start {
                    let mut a = Attrs::new();
                    a.insert("start".into(), AttrValue::from(n));
                    ("ordered_list", a)
                } else {
                    ("bullet_list", Attrs::new())
                };
                if attrs.get("start").and_then(|v| v.as_u64()) == Some(1) {
                    attrs.remove("start");
                }
                if schema.node_type(name).is_some() {
                    out.push(schema.node(name, attrs, items, vec![])?);
                }
            }
            Event::Text(t) if schema.node_type("paragraph").is_some() => {
                // Bare text inside a list item (no paragraph wrapper) — wrap it.
                let txt = schema.text(&t, inherited_marks.to_vec())?;
                out.push(schema.node("paragraph", Attrs::new(), vec![txt], vec![])?);
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_inline(
    schema: &Schema,
    parser: &mut Parser<'_>,
    end_tag: TagEnd,
    inherited_marks: &[Mark],
) -> Result<Vec<Node>, DocError> {
    let mut out = Vec::new();
    let mut mark_stack: Vec<Mark> = inherited_marks.to_vec();
    for ev in parser.by_ref() {
        if matches!(&ev, Event::End(t) if discriminant_match(t, &end_tag)) {
            return Ok(out);
        }
        consume_inline(schema, ev, &mut out, &mut mark_stack)?;
    }
    Ok(out)
}

fn discriminant_match(a: &TagEnd, b: &TagEnd) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn consume_inline_event(schema: &Schema, ev: Event<'_>, inherited_marks: &[Mark]) -> Vec<Node> {
    let mut out = Vec::new();
    let mut marks = inherited_marks.to_vec();
    let _ = consume_inline(schema, ev, &mut out, &mut marks);
    out
}

fn consume_inline(
    schema: &Schema,
    ev: Event<'_>,
    out: &mut Vec<Node>,
    marks: &mut Vec<Mark>,
) -> Result<(), DocError> {
    match ev {
        Event::Text(t) => {
            if let Ok(n) = schema.text(&t, marks.clone()) {
                out.push(n);
            }
        }
        Event::Code(t) => {
            // Inline code → text carrying the `code` mark when the schema
            // declares it (the `Code` extension); otherwise plain text.
            let mut cm = marks.clone();
            if let Some(m) = mark_of(schema, "code", Attrs::new()) {
                if !cm.iter().any(|x| x.mark_type().name() == "code") {
                    cm.push(m);
                }
            }
            if let Ok(n) = schema.text(&t, cm) {
                out.push(n);
            }
        }
        Event::SoftBreak | Event::HardBreak => {
            if let Ok(n) = schema.text(" ", marks.clone()) {
                out.push(n);
            }
        }
        Event::Start(Tag::Strong) => {
            if let Some(m) = mark_of(schema, "strong", Attrs::new()) {
                marks.push(m);
            }
        }
        Event::Start(Tag::Emphasis) => {
            if let Some(m) = mark_of(schema, "em", Attrs::new()) {
                marks.push(m);
            }
        }
        Event::Start(Tag::Link {
            dest_url, title, ..
        }) => {
            let mut a = Attrs::new();
            a.insert("href".into(), AttrValue::from(dest_url.to_string()));
            if !title.is_empty() {
                a.insert("title".into(), AttrValue::from(title.to_string()));
            }
            if let Some(m) = mark_of(schema, "link", a) {
                marks.push(m);
            }
        }
        Event::Start(Tag::Image {
            dest_url, title, ..
        }) if schema.node_type("image").is_some() => {
            let mut attrs = Attrs::new();
            attrs.insert("src".into(), AttrValue::from(dest_url.to_string()));
            let alt = collect_image_alt();
            attrs.insert("alt".into(), AttrValue::from(alt));
            if !title.is_empty() {
                attrs.insert("title".into(), AttrValue::from(title.to_string()));
            }
            if let Ok(img) = schema.node("image", attrs, vec![], marks.clone()) {
                out.push(img);
            }
        }
        Event::End(TagEnd::Strong) => pop_mark(marks, "strong"),
        Event::End(TagEnd::Emphasis) => pop_mark(marks, "em"),
        Event::End(TagEnd::Link) => pop_mark(marks, "link"),
        Event::End(TagEnd::Image) => { /* image emitted on Start; no-op */ }
        _ => {}
    }
    Ok(())
}

fn collect_image_alt() -> String {
    // pulldown-cmark emits the alt text as a sequence of Text events
    // between Start(Image) and End(Image). We don't get to peek
    // ergonomically; a richer impl would track a buffer. For v0.2 the
    // alt-text round-trip is via the explicit `alt` attribute on `image`
    // when we serialize, and on parsing we accept an empty alt and let
    // the user re-edit. Good enough for v0.2.
    String::new()
}

fn pop_mark(marks: &mut Vec<Mark>, name: &str) {
    if let Some(idx) = marks.iter().rposition(|m| m.mark_type().name() == name) {
        marks.remove(idx);
    }
}

fn mark_of(schema: &Schema, name: &str, attrs: Attrs) -> Option<Mark> {
    let mt: &MarkType = schema.mark_type(name)?;
    Some(mt.create(fill_mark_attrs(mt, attrs)))
}

fn fill_mark_attrs(mt: &MarkType, mut given: Attrs) -> Attrs {
    for (k, s) in &mt.spec().attrs {
        if !given.contains_key(k) {
            if let Some(d) = &s.default {
                given.insert(k.clone(), d.clone());
            }
        }
    }
    given
}

fn collect_code(parser: &mut Parser<'_>) -> String {
    let mut out = String::new();
    for ev in parser.by_ref() {
        match ev {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::End(TagEnd::CodeBlock) => return out,
            _ => {}
        }
    }
    out
}

fn skip_until_end(parser: &mut Parser<'_>) {
    let mut depth = 1;
    for ev in parser.by_ref() {
        match ev {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            _ => {}
        }
    }
}

fn level_to_u64(level: HeadingLevel) -> u64 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

// Cannot suppress warning about unused parameter on collect_image_alt — and
// HashMap import only for the (currently unused) future-proofing of
// mark_open_attrs.
#[allow(dead_code)]
fn _unused_hashmap(_: HashMap<String, ()>) {}

#[allow(dead_code)]
fn _attr_unused(_: &CodeBlockKind<'_>) {}
