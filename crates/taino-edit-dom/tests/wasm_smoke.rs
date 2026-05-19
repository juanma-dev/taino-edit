//! Pipeline smoke test: proves `taino-edit-dom` builds to wasm, runs in a
//! real (headless Chromium) browser, can touch the DOM via `web-sys`, and
//! that `taino-edit-core` links and works under `wasm32`.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn dom_is_available() {
    let document = web_sys::window().unwrap().document().unwrap();
    let div = document.create_element("div").unwrap();
    div.set_text_content(Some("hola chromium"));
    assert_eq!(div.text_content().unwrap(), "hola chromium");
    assert_eq!(div.tag_name().to_lowercase(), "div");
}

#[wasm_bindgen_test]
fn core_links_and_runs_under_wasm() {
    use taino_edit_core::{NodeSpec, SchemaBuilder};

    let schema = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("paragraph+".into()),
                ..Default::default()
            },
        )
        .node(
            "paragraph",
            NodeSpec {
                content: Some("text*".into()),
                ..Default::default()
            },
        )
        .node("text", NodeSpec::default())
        .top_node("doc")
        .build()
        .unwrap();

    let text = schema.text("hi", vec![]).unwrap();
    let para = schema
        .node("paragraph", Default::default(), vec![text], vec![])
        .unwrap();
    let doc = schema
        .node("doc", Default::default(), vec![para], vec![])
        .unwrap();
    assert_eq!(doc.text_content(), "hi");
}
