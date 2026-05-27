//! The `schema!` macro is sugar over `SchemaBuilder`: these tests prove it
//! produces an equivalent schema and that each supported key wires through.

use taino_edit_core::{schema, Attrs, DomSpec, NodeSpec, ParseRule, Schema, SchemaBuilder};

/// Build the same schema by hand for an equivalence check.
fn built_by_hand() -> Schema {
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
        .top_node("doc")
        .build()
        .unwrap()
}

#[test]
fn macro_matches_the_hand_built_builder() {
    let by_macro = schema! {
        top: "doc",
        nodes: {
            doc { content: "block+" },
            paragraph { content: "inline*", group: "block", dom: "p" },
            text { group: "inline" },
        },
    }
    .unwrap();

    let by_hand = built_by_hand();

    // Same node types, in the same declaration order.
    let names: Vec<&str> = by_macro.node_types().iter().map(|n| n.name()).collect();
    let hand_names: Vec<&str> = by_hand.node_types().iter().map(|n| n.name()).collect();
    assert_eq!(names, hand_names);
    assert_eq!(by_macro.top_node_type().name(), "doc");

    // And it produces a usable schema: a paragraph renders to <p>.
    let p = by_macro
        .node(
            "paragraph",
            Default::default(),
            vec![by_macro.text("hi", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    assert_eq!(p.to_html(), "<p>hi</p>");
}

#[test]
fn macro_supports_marks_attrs_parse_and_explicit_to_dom() {
    let s = schema! {
        nodes: {
            doc { content: "block+" },
            paragraph { content: "inline*", group: "block", dom: "p", parse: ["p"] },
            heading {
                content: "inline*",
                group: "block",
                attrs: { level: 1 },
                to_dom: |n| {
                    let l = n.attrs().get("level").and_then(|v| v.as_u64()).unwrap_or(1);
                    DomSpec::element(&format!("h{l}"))
                },
                parse: ["h1", "h2", "h3"],
            },
            text { group: "inline" },
        },
        marks: {
            strong { dom: "strong", parse: ["strong", "b"], inclusive: true },
            em { dom: "em" },
        },
    }
    .unwrap();

    // Top node defaults to "doc" when `top:` is omitted.
    assert_eq!(s.top_node_type().name(), "doc");

    // Marks landed.
    assert!(s.mark_type("strong").is_some());
    assert!(s.mark_type("em").is_some());

    // The `attrs` block gave `level` a default of 1, used when omitted.
    let h = s
        .node(
            "heading",
            Attrs::new(),
            vec![s.text("Title", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    assert_eq!(h.to_html(), "<h1>Title</h1>");

    // The explicit `to_dom` closure honours an overridden level.
    let h2 = s
        .node(
            "heading",
            Attrs::from_iter([("level".into(), serde_json::json!(2))]),
            vec![s.text("Sub", vec![]).unwrap()],
            vec![],
        )
        .unwrap();
    assert_eq!(h2.to_html(), "<h2>Sub</h2>");

    // `parse` shorthand maps each tag to the node type (no attr extraction,
    // so the level isn't recovered — that needs a custom ParseRule).
    let parsed = s.parse_html("<h3>X</h3>").unwrap();
    let first = parsed.content().iter().next().unwrap();
    assert_eq!(first.node_type().name(), "heading");
    // The mark `parse` shorthand maps the alias `<b>` onto `strong`.
    let bold = s.parse_html("<p><b>hi</b></p>").unwrap();
    assert!(bold.to_html().contains("<strong>hi</strong>"));
}

#[test]
fn trailing_commas_are_optional() {
    // No trailing commas anywhere. `content: "text*"` references the `text`
    // node directly, so this minimal schema is valid on its own.
    let s = schema! {
        nodes: {
            doc { content: "text*" },
            text { group: "inline" }
        }
    }
    .unwrap();
    assert_eq!(s.node_types().len(), 2);
    let _ = ParseRule::tag("p"); // keep the import meaningful
}
