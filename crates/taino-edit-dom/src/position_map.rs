//! Bidirectional mapping between document positions and DOM `(node, offset)`
//! points, driven by the [`ViewDesc`] tree mounted on the editor's root.
//!
//! v0.1 covers the cases an editor actually needs:
//!
//! * inter-block boundaries at the root → `(root, child_index)`;
//! * positions inside a textblock at a child boundary → `(element, child_index)`;
//! * positions inside a text run → `(text_node, char_offset)`.
//!
//! The inverse maps `(root|element, child_index)` and `(text_node, offset)`
//! back to absolute document positions.

use wasm_bindgen::JsValue;
use web_sys::Element;

use crate::desc::ViewDesc;

fn node_eq(a: &web_sys::Node, b: &web_sys::Node) -> bool {
    JsValue::from(a) == JsValue::from(b)
}

/// Map document position `pos` to a DOM selection point.
pub fn doc_pos_to_dom(
    root: &Element,
    children: &[ViewDesc],
    pos: usize,
) -> Option<(web_sys::Node, u32)> {
    let mut cur = 0usize;
    for (idx, desc) in children.iter().enumerate() {
        let nsize = desc.node().node_size();
        if pos == cur {
            return Some((root.clone().into(), idx as u32));
        }
        if pos > cur && pos < cur + nsize {
            return resolve_inside(desc, pos - cur - 1);
        }
        cur += nsize;
    }
    if pos == cur {
        return Some((root.clone().into(), children.len() as u32));
    }
    None
}

fn resolve_inside(desc: &ViewDesc, content_offset: usize) -> Option<(web_sys::Node, u32)> {
    match desc {
        ViewDesc::Text { text, .. } => Some((text.clone().into(), content_offset as u32)),
        ViewDesc::Element { dom, children, .. } => {
            let mut cur = 0usize;
            for (idx, c) in children.iter().enumerate() {
                let csize = c.node().node_size();
                if content_offset == cur {
                    return Some((dom.clone().into(), idx as u32));
                }
                if content_offset > cur && content_offset < cur + csize {
                    // Offset into `c`, measured from its start. Descending into
                    // an element child skips its opening token (text nodes have
                    // none), so subtract 1 for elements.
                    let inner = content_offset - cur;
                    let inner = match c {
                        ViewDesc::Element { .. } => inner - 1,
                        ViewDesc::Text { .. } => inner,
                    };
                    return resolve_inside(c, inner);
                }
                cur += csize;
            }
            if content_offset == cur {
                return Some((dom.clone().into(), children.len() as u32));
            }
            None
        }
    }
}

/// Map a DOM `(node, offset)` point back to an absolute document position.
pub fn dom_to_doc_pos(
    root: &Element,
    children: &[ViewDesc],
    dom_node: &web_sys::Node,
    offset: u32,
) -> Option<usize> {
    let root_node: web_sys::Node = root.clone().into();
    if node_eq(&root_node, dom_node) {
        let mut pos = 0;
        for (i, desc) in children.iter().enumerate() {
            if i == offset as usize {
                return Some(pos);
            }
            pos += desc.node().node_size();
        }
        return Some(pos);
    }

    let mut pos = 0usize;
    for desc in children {
        if let Some(p) = inside_for_dom(desc, dom_node, offset, pos + 1) {
            return Some(p);
        }
        pos += desc.node().node_size();
    }
    None
}

fn inside_for_dom(
    desc: &ViewDesc,
    target: &web_sys::Node,
    offset: u32,
    pos_at_content_start: usize,
) -> Option<usize> {
    match desc {
        ViewDesc::Text {
            text,
            wrapper,
            node,
        } => {
            let tn: web_sys::Node = text.clone().into();
            if node_eq(&tn, target) {
                return Some(pos_at_content_start + offset as usize);
            }
            if let Some(w) = wrapper {
                let wn: web_sys::Node = w.clone().into();
                if node_eq(&wn, target) {
                    let len = node.node_size();
                    return Some(if offset == 0 {
                        pos_at_content_start
                    } else {
                        pos_at_content_start + len
                    });
                }
            }
            None
        }
        ViewDesc::Element { dom, children, .. } => {
            let dn: web_sys::Node = dom.clone().into();
            if node_eq(&dn, target) {
                let mut pos = pos_at_content_start;
                for (i, c) in children.iter().enumerate() {
                    if i == offset as usize {
                        return Some(pos);
                    }
                    pos += c.node().node_size();
                }
                return Some(pos);
            }
            let mut pos = pos_at_content_start;
            for c in children {
                // A child element's content starts one past its own position
                // (its opening token); a text child's chars start at `pos`.
                let child_start = match c {
                    ViewDesc::Element { .. } => pos + 1,
                    ViewDesc::Text { .. } => pos,
                };
                if let Some(p) = inside_for_dom(c, target, offset, child_start) {
                    return Some(p);
                }
                pos += c.node().node_size();
            }
            None
        }
    }
}
