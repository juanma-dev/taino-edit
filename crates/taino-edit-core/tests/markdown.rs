//! v0.2 — Markdown serializer + parser round-trip.

use taino_edit_core::{
    markdown::{parse_markdown, to_markdown},
    AttrValue, Attrs, MarkSpec, Node, NodeSpec, ParseRule, Schema, SchemaBuilder,
};

fn schema() -> Schema {
    SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "paragraph",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "heading",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                attrs: {
                    let mut a = std::collections::HashMap::new();
                    a.insert(
                        "level".into(),
                        taino_edit_core::AttrSpec {
                            default: Some(AttrValue::from(1u64)),
                        },
                    );
                    a
                },
                ..Default::default()
            },
        )
        .node(
            "blockquote",
            NodeSpec {
                content: Some("block+".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "code_block",
            NodeSpec {
                content: Some("text*".into()),
                group: Some("block".into()),
                marks: Some(String::new()),
                ..Default::default()
            },
        )
        .node(
            "list_item",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "bullet_list",
            NodeSpec {
                content: Some("list_item+".into()),
                group: Some("block".into()),
                ..Default::default()
            },
        )
        .node(
            "ordered_list",
            NodeSpec {
                content: Some("list_item+".into()),
                group: Some("block".into()),
                attrs: {
                    let mut a = std::collections::HashMap::new();
                    a.insert(
                        "start".into(),
                        taino_edit_core::AttrSpec {
                            default: Some(AttrValue::from(1u64)),
                        },
                    );
                    a
                },
                ..Default::default()
            },
        )
        .node(
            "image",
            NodeSpec {
                group: Some("inline".into()),
                inline: true,
                atom: true,
                attrs: {
                    let mut a = std::collections::HashMap::new();
                    a.insert(
                        "src".into(),
                        taino_edit_core::AttrSpec {
                            default: Some(AttrValue::from(String::new())),
                        },
                    );
                    a.insert(
                        "alt".into(),
                        taino_edit_core::AttrSpec {
                            default: Some(AttrValue::from(String::new())),
                        },
                    );
                    a
                },
                parse_dom: vec![ParseRule::tag("img")],
                ..Default::default()
            },
        )
        .node(
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        )
        .mark("strong", MarkSpec::default())
        .mark("em", MarkSpec::default())
        .mark("code", MarkSpec::default())
        .mark(
            "link",
            MarkSpec {
                attrs: {
                    let mut a = std::collections::HashMap::new();
                    a.insert(
                        "href".into(),
                        taino_edit_core::AttrSpec {
                            default: Some(AttrValue::from(String::new())),
                        },
                    );
                    a.insert(
                        "title".into(),
                        taino_edit_core::AttrSpec {
                            default: Some(AttrValue::Null),
                        },
                    );
                    a
                },
                ..Default::default()
            },
        )
        .top_node("doc")
        .build()
        .unwrap()
}

fn p(s: &Schema, text: &str) -> Node {
    let t = s.text(text, vec![]).unwrap();
    s.node("paragraph", Default::default(), vec![t], vec![])
        .unwrap()
}

// ---- Serializer tests ----------------------------------------------------

#[test]
fn paragraph_serializes_to_plain_text() {
    let s = schema();
    let doc = s
        .node("doc", Default::default(), vec![p(&s, "hello")], vec![])
        .unwrap();
    assert_eq!(to_markdown(&doc).trim_end(), "hello");
}

#[test]
fn heading_emits_hashes() {
    let s = schema();
    let t = s.text("Title", vec![]).unwrap();
    let mut attrs = Attrs::new();
    attrs.insert("level".into(), AttrValue::from(2u64));
    let h = s.node("heading", attrs, vec![t], vec![]).unwrap();
    let doc = s.node("doc", Default::default(), vec![h], vec![]).unwrap();
    assert_eq!(to_markdown(&doc).trim_end(), "## Title");
}

#[test]
fn strong_and_em_emit_stars() {
    let s = schema();
    let strong = s.mark_type("strong").unwrap().clone();
    let em = s.mark_type("em").unwrap().clone();
    let bold_run = s.text("bold", vec![strong.create(Attrs::new())]).unwrap();
    let italic_run = s
        .text(" and italic", vec![em.create(Attrs::new())])
        .unwrap();
    let para = s
        .node(
            "paragraph",
            Default::default(),
            vec![bold_run, italic_run],
            vec![],
        )
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![para], vec![])
        .unwrap();
    let md = to_markdown(&doc);
    assert!(md.contains("**bold**"));
    assert!(md.contains("*and italic*") || md.contains("* and italic*"));
}

#[test]
fn link_round_trips_via_brackets_and_parens() {
    let s = schema();
    let link = s.mark_type("link").unwrap().clone();
    let mut href = Attrs::new();
    href.insert("href".into(), AttrValue::from("https://example.com"));
    let run = s.text("click", vec![link.create(href)]).unwrap();
    let para = s
        .node("paragraph", Default::default(), vec![run], vec![])
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![para], vec![])
        .unwrap();
    let md = to_markdown(&doc);
    assert!(md.contains("[click](https://example.com)"), "got: {md}");
}

#[test]
fn bullet_list_emits_dash_prefix() {
    let s = schema();
    let item = |t: &str| {
        s.node("list_item", Default::default(), vec![p(&s, t)], vec![])
            .unwrap()
    };
    let ul = s
        .node(
            "bullet_list",
            Default::default(),
            vec![item("a"), item("b")],
            vec![],
        )
        .unwrap();
    let doc = s.node("doc", Default::default(), vec![ul], vec![]).unwrap();
    let md = to_markdown(&doc);
    assert!(md.contains("- a\n"), "got: {md}");
    assert!(md.contains("- b\n"), "got: {md}");
}

#[test]
fn ordered_list_emits_numeric_prefix() {
    let s = schema();
    let item = |t: &str| {
        s.node("list_item", Default::default(), vec![p(&s, t)], vec![])
            .unwrap()
    };
    let ol = s
        .node(
            "ordered_list",
            Default::default(),
            vec![item("a"), item("b"), item("c")],
            vec![],
        )
        .unwrap();
    let doc = s.node("doc", Default::default(), vec![ol], vec![]).unwrap();
    let md = to_markdown(&doc);
    assert!(md.contains("1. a"), "got: {md}");
    assert!(md.contains("2. b"), "got: {md}");
    assert!(md.contains("3. c"), "got: {md}");
}

#[test]
fn blockquote_lines_prefixed_with_greater_than() {
    let s = schema();
    let bq = s
        .node(
            "blockquote",
            Default::default(),
            vec![p(&s, "quoted")],
            vec![],
        )
        .unwrap();
    let doc = s.node("doc", Default::default(), vec![bq], vec![]).unwrap();
    let md = to_markdown(&doc);
    assert!(md.contains("> quoted"), "got: {md}");
}

#[test]
fn code_block_uses_fenced_triple_backtick() {
    let s = schema();
    let t = s.text("let x = 1;", vec![]).unwrap();
    let cb = s
        .node("code_block", Default::default(), vec![t], vec![])
        .unwrap();
    let doc = s.node("doc", Default::default(), vec![cb], vec![]).unwrap();
    let md = to_markdown(&doc);
    assert!(md.contains("```\nlet x = 1;\n```"), "got: {md}");
}

#[test]
fn image_emits_bang_link_form() {
    let s = schema();
    let mut a = Attrs::new();
    a.insert("src".into(), AttrValue::from("x.png"));
    a.insert("alt".into(), AttrValue::from("an X"));
    let img = s.node("image", a, vec![], vec![]).unwrap();
    let para = s
        .node("paragraph", Default::default(), vec![img], vec![])
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![para], vec![])
        .unwrap();
    let md = to_markdown(&doc);
    assert!(md.contains("![an X](x.png)"), "got: {md}");
}

// ---- Parser tests --------------------------------------------------------

#[test]
fn parse_plain_paragraph() {
    let s = schema();
    let doc = parse_markdown(&s, "hello world").unwrap();
    assert_eq!(doc.child(0).node_type().name(), "paragraph");
    assert_eq!(doc.text_content(), "hello world");
}

#[test]
fn parse_heading_levels() {
    let s = schema();
    let doc = parse_markdown(&s, "# H1\n\n## H2\n\n### H3\n").unwrap();
    assert_eq!(doc.child_count(), 3);
    for (i, expected) in [(0, 1u64), (1, 2), (2, 3)] {
        let child = doc.child(i);
        assert_eq!(child.node_type().name(), "heading");
        assert_eq!(child.attrs().get("level"), Some(&AttrValue::from(expected)));
    }
}

#[test]
fn parse_bold_and_italic() {
    let s = schema();
    let doc = parse_markdown(&s, "**bold** and *em*").unwrap();
    let kids: Vec<&str> = doc
        .child(0)
        .content()
        .iter()
        .map(|n| n.text().unwrap_or(""))
        .collect();
    assert!(kids.contains(&"bold"));
    assert!(kids.contains(&"em"));
}

#[test]
fn parse_link() {
    let s = schema();
    let doc = parse_markdown(&s, "[click](https://example.com)").unwrap();
    let txt = doc.child(0).child(0);
    let marks = txt.marks();
    assert!(marks.iter().any(|m| m.mark_type().name() == "link"
        && m.attrs().get("href") == Some(&AttrValue::from("https://example.com"))));
}

#[test]
fn parse_bullet_list() {
    let s = schema();
    let doc = parse_markdown(&s, "- a\n- b\n").unwrap();
    let ul = doc.child(0);
    assert_eq!(ul.node_type().name(), "bullet_list");
    assert_eq!(ul.child_count(), 2);
}

#[test]
fn parse_ordered_list() {
    let s = schema();
    let doc = parse_markdown(&s, "1. a\n2. b\n").unwrap();
    let ol = doc.child(0);
    assert_eq!(ol.node_type().name(), "ordered_list");
    assert_eq!(ol.child_count(), 2);
}

#[test]
fn parse_blockquote() {
    let s = schema();
    let doc = parse_markdown(&s, "> quoted\n").unwrap();
    assert_eq!(doc.child(0).node_type().name(), "blockquote");
}

#[test]
fn parse_code_block() {
    let s = schema();
    let doc = parse_markdown(&s, "```\nlet x = 1;\n```\n").unwrap();
    let cb = doc.child(0);
    assert_eq!(cb.node_type().name(), "code_block");
    assert_eq!(cb.text_content(), "let x = 1;\n");
}

#[test]
fn round_trip_paragraph_with_bold() {
    let s = schema();
    let doc = parse_markdown(&s, "**hi**").unwrap();
    let md = to_markdown(&doc);
    assert!(md.trim_end().contains("**hi**"), "got: {md}");
}

#[test]
fn round_trip_heading() {
    let s = schema();
    let doc = parse_markdown(&s, "## Title").unwrap();
    let md = to_markdown(&doc);
    assert_eq!(md.trim_end(), "## Title");
}

#[test]
fn round_trip_bullet_list() {
    let s = schema();
    let doc = parse_markdown(&s, "- a\n- b\n").unwrap();
    let md = to_markdown(&doc);
    assert!(md.contains("- a"));
    assert!(md.contains("- b"));
}

#[test]
fn parse_inline_code_applies_code_mark() {
    let s = schema();
    let doc = parse_markdown(&s, "run `cargo test` now").unwrap();
    // The "cargo test" run carries the code mark.
    let para = doc.child(0);
    let coded = para
        .content()
        .iter()
        .find(|n| n.text() == Some("cargo test"))
        .expect("the code run is present");
    assert!(
        coded.marks().iter().any(|m| m.mark_type().name() == "code"),
        "inline code should carry the code mark"
    );
}

#[test]
fn round_trip_inline_code() {
    let s = schema();
    let doc = parse_markdown(&s, "use `x` here").unwrap();
    let md = to_markdown(&doc);
    assert!(
        md.contains("`x`"),
        "inline code round-trips to backticks: {md}"
    );
    // And the literal text inside backticks is not markdown-escaped.
    let doc2 = parse_markdown(&s, "call `a*b`").unwrap();
    let md2 = to_markdown(&doc2);
    assert!(md2.contains("`a*b`"), "code content stays literal: {md2}");
}
