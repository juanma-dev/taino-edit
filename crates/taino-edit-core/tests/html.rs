//! Phase 1 HTML acceptance: doc → HTML → doc round-trips, output is escaped,
//! parsing is schema-strict, and hostile input is bounded.

use serde_json::json;
use taino_edit_core::{
    Attrs, DocError, DomSpec, HtmlElement, MarkSpec, Node, NodeSpec, ParseRule, Schema,
    SchemaBuilder, MAX_DEPTH,
};

fn level_attrs(n: u64) -> Option<Attrs> {
    let mut a = Attrs::new();
    a.insert("level".into(), json!(n));
    Some(a)
}

fn img_attrs(el: &HtmlElement) -> Option<Attrs> {
    let mut a = Attrs::new();
    a.insert("src".into(), json!(el.attr("src").unwrap_or("")));
    Some(a)
}

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
                to_dom: Some(|_| DomSpec::element("p")),
                parse_dom: vec![ParseRule::tag("p")],
                ..Default::default()
            },
        )
        .node(
            "heading",
            NodeSpec {
                content: Some("inline*".into()),
                group: Some("block".into()),
                attrs: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "level".to_string(),
                        taino_edit_core::AttrSpec {
                            default: Some(json!(1)),
                        },
                    );
                    m
                },
                to_dom: Some(|n| {
                    let lvl = n.attrs().get("level").and_then(|v| v.as_u64()).unwrap_or(1);
                    DomSpec::element(&format!("h{lvl}"))
                }),
                parse_dom: vec![
                    ParseRule::with_attrs("h1", |_| level_attrs(1)),
                    ParseRule::with_attrs("h2", |_| level_attrs(2)),
                    ParseRule::with_attrs("h3", |_| level_attrs(3)),
                ],
                ..Default::default()
            },
        )
        .node(
            "image",
            NodeSpec {
                group: Some("inline".into()),
                inline: true,
                attrs: {
                    let mut m = std::collections::HashMap::new();
                    m.insert(
                        "src".to_string(),
                        taino_edit_core::AttrSpec {
                            default: Some(json!("")),
                        },
                    );
                    m
                },
                to_dom: Some(|n| {
                    let src = n.attrs().get("src").and_then(|v| v.as_str()).unwrap_or("");
                    DomSpec::void("img").attr("src", src)
                }),
                parse_dom: vec![ParseRule::with_attrs("img", img_attrs)],
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
        .mark(
            "strong",
            MarkSpec {
                to_dom: Some(|_| DomSpec::element("strong")),
                parse_dom: vec![ParseRule::tag("strong"), ParseRule::tag("b")],
                ..Default::default()
            },
        )
        .mark(
            "em",
            MarkSpec {
                to_dom: Some(|_| DomSpec::element("em")),
                parse_dom: vec![ParseRule::tag("em")],
                ..Default::default()
            },
        )
        .top_node("doc")
        .build()
        .expect("schema builds")
}

fn doc_with_image(s: &Schema, src: &str) -> Node {
    let title = s.text("Title", vec![]).unwrap();
    let heading = s
        .node("heading", Default::default(), vec![title], vec![])
        .unwrap();

    let strong = s.mark_type("strong").unwrap().create(Default::default());
    let hello = s.text("Hello ", vec![]).unwrap();
    let world = s.text("world", vec![strong]).unwrap();
    let mut img_attrs = Attrs::new();
    img_attrs.insert("src".into(), json!(src));
    let img = s.node("image", img_attrs, vec![], vec![]).unwrap();
    let para = s
        .node(
            "paragraph",
            Default::default(),
            vec![hello, world, img],
            vec![],
        )
        .unwrap();

    s.node("doc", Default::default(), vec![heading, para], vec![])
        .unwrap()
}

#[test]
fn html_round_trips() {
    let s = schema();
    let doc = doc_with_image(&s, "/cat.png");

    let html = doc.to_html();
    assert_eq!(
        html,
        "<h1>Title</h1><p>Hello <strong>world</strong><img src=\"/cat.png\"/></p>"
    );

    let back = s.parse_html(&html).expect("re-parse");
    assert_eq!(doc, back, "doc must survive an HTML round-trip");
}

#[test]
fn output_is_escaped() {
    let s = schema();

    let txt = s.text("a < b & c > d \" e", vec![]).unwrap();
    let para = s
        .node("paragraph", Default::default(), vec![txt], vec![])
        .unwrap();
    let doc = s
        .node("doc", Default::default(), vec![para], vec![])
        .unwrap();

    let html = doc.to_html();
    assert!(html.contains("a &lt; b &amp; c &gt; d \" e"));
    assert!(!html.contains("a < b"), "raw markup must not leak");

    // Attribute values are escaped too, and survive the round-trip.
    let hostile_src = "/x?a=1&b=2\"><script>alert(1)</script>";
    let doc2 = doc_with_image(&s, hostile_src);
    let html2 = doc2.to_html();
    assert!(html2.contains("&quot;"));
    assert!(html2.contains("&lt;script&gt;"));
    assert!(
        !html2.contains("\"><script>"),
        "must not be able to break out of the src attribute"
    );
    assert_eq!(s.parse_html(&html2).unwrap(), doc2);
}

#[test]
fn entities_are_decoded() {
    let s = schema();
    let doc = s
        .parse_html("<p>&#65;&#x42;&amp;&lt;&gt;&quot;ok</p>")
        .unwrap();
    assert_eq!(doc.text_content(), "AB&<>\"ok");
}

#[test]
fn parsing_is_schema_strict() {
    let s = schema();

    // Unknown wrapper elements are unwrapped, content preserved.
    let d = s
        .parse_html("<section><div><p>kept</p></div></section>")
        .unwrap();
    assert_eq!(d.child_count(), 1);
    assert_eq!(d.child(0).node_type().name(), "paragraph");
    assert_eq!(d.text_content(), "kept");

    // A block inside a heading violates `inline*`.
    assert!(matches!(
        s.parse_html("<h1><p>no</p></h1>"),
        Err(DocError::InvalidContent { .. })
    ));

    // <b> is mapped to the `strong` mark by an alternate parse rule.
    let bold = s.parse_html("<p><b>x</b></p>").unwrap();
    assert_eq!(
        bold.child(0).child(0).marks()[0].mark_type().name(),
        "strong"
    );
}

#[test]
fn tolerates_messy_but_safe_html() {
    let s = schema();

    // Doctype + comment skipped; insignificant whitespace dropped.
    let d = s
        .parse_html("<!DOCTYPE html><!-- hi --><p>a</p>\n   <p>b</p>")
        .unwrap();
    assert_eq!(d.child_count(), 2);
    assert_eq!(d.text_content(), "ab");

    // Unclosed tag auto-closed; stray close ignored.
    let d2 = s.parse_html("<p>open</div>").unwrap();
    assert_eq!(d2.child(0).node_type().name(), "paragraph");
    assert_eq!(d2.text_content(), "open");
}

#[test]
fn hostile_deep_nesting_is_rejected() {
    let s = schema();
    let deep = format!(
        "{}<p>x</p>{}",
        "<span>".repeat(MAX_DEPTH + 5),
        "</span>".repeat(MAX_DEPTH + 5)
    );
    assert!(matches!(s.parse_html(&deep), Err(DocError::HtmlParse(_))));
}
