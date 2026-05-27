//! The [`schema!`](crate::schema) declarative macro — ergonomic sugar over
//! [`SchemaBuilder`](crate::SchemaBuilder).
//!
//! Per DESIGN_NOTES §6 this is a `macro_rules!` macro, *not* a proc-macro:
//! it expands to the same builder calls you would write by hand (so it adds
//! no dependencies and no new architecture), it just removes the
//! `NodeSpec { .. ..Default::default() }` boilerplate and the final
//! `.node(..)` / `.build()` chaining.
//!
//! ```
//! use taino_edit_core::{schema, DomSpec};
//!
//! let s = schema! {
//!     top: "doc",
//!     nodes: {
//!         doc { content: "block+" },
//!         paragraph { content: "inline*", group: "block", dom: "p" },
//!         heading {
//!             content: "inline*",
//!             group: "block",
//!             attrs: { level: 1 },
//!             to_dom: |n| {
//!                 let l = n.attrs().get("level").and_then(|v| v.as_u64()).unwrap_or(1);
//!                 DomSpec::element(&format!("h{l}"))
//!             },
//!             parse: ["h1", "h2", "h3"],
//!         },
//!         text { group: "inline" },
//!     },
//!     marks: {
//!         strong { dom: "strong", parse: ["strong", "b"] },
//!         em { dom: "em", parse: ["em", "i"] },
//!     },
//! }
//! .expect("schema builds");
//!
//! assert!(s.node_type("heading").is_some());
//! assert!(s.mark_type("strong").is_some());
//! ```
//!
//! ## Supported keys
//!
//! Each node/mark body is a brace block of `key: value` pairs; every key is
//! optional and unrecognised keys are a compile error.
//!
//! * **nodes** — `content`, `group`, `marks` (strings); `inline`, `atom`
//!   (bools); `dom` (a tag name, shorthand for an element renderer with no
//!   attrs); `to_dom` (an explicit `fn(&Node) -> DomSpec` / non-capturing
//!   closure); `parse` (a list of tag-name strings); `attrs`
//!   (a `{ name: default, .. }` block).
//! * **marks** — `group` (string); `inclusive` (bool); `dom`; `to_dom`
//!   (`fn(&Mark) -> DomSpec`); `parse`; `attrs`.
//!
//! Sections are written in the order `top?, nodes, marks?`. The macro yields a
//! `Result<Schema, SchemaError>` (it calls `.build()` for you).

/// Build a [`Schema`](crate::Schema) from a compact declaration. See the
/// [module docs](crate::schema_macro) for the supported syntax.
#[macro_export]
macro_rules! schema {
    // ---- entry --------------------------------------------------------
    (
        $(top: $top:expr,)?
        nodes: { $($nodes:tt)* }
        $(, marks: { $($marks:tt)* })?
        $(,)?
    ) => {{
        #[allow(unused_mut)]
        let mut __builder = $crate::SchemaBuilder::new();
        $( __builder = __builder.top_node($top); )?
        __builder = $crate::schema!(@nodes __builder, $($nodes)*);
        $( __builder = $crate::schema!(@marks __builder, $($marks)*); )?
        __builder.build()
    }};

    // ---- node list ----------------------------------------------------
    (@nodes $b:expr,) => { $b };
    (@nodes $b:expr, $name:ident { $($fields:tt)* } $(, $($rest:tt)*)?) => {{
        #[allow(unused_mut)]
        let mut __spec = $crate::NodeSpec::default();
        $crate::schema!(@node_fields __spec, $($fields)*);
        let __b = $b.node(stringify!($name), __spec);
        $crate::schema!(@nodes __b, $($($rest)*)?)
    }};

    // ---- node fields --------------------------------------------------
    (@node_fields $s:ident,) => {};
    (@node_fields $s:ident, content: $v:expr $(, $($r:tt)*)?) => {
        $s.content = ::core::option::Option::Some($v.into());
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };
    (@node_fields $s:ident, group: $v:expr $(, $($r:tt)*)?) => {
        $s.group = ::core::option::Option::Some($v.into());
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };
    (@node_fields $s:ident, marks: $v:expr $(, $($r:tt)*)?) => {
        $s.marks = ::core::option::Option::Some($v.into());
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };
    (@node_fields $s:ident, inline: $v:expr $(, $($r:tt)*)?) => {
        $s.inline = $v;
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };
    (@node_fields $s:ident, atom: $v:expr $(, $($r:tt)*)?) => {
        $s.atom = $v;
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };
    (@node_fields $s:ident, dom: $v:expr $(, $($r:tt)*)?) => {
        $s.to_dom = ::core::option::Option::Some(|_| $crate::DomSpec::element($v));
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };
    (@node_fields $s:ident, to_dom: $v:expr $(, $($r:tt)*)?) => {
        $s.to_dom = ::core::option::Option::Some($v);
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };
    (@node_fields $s:ident, parse: [ $($t:expr),* $(,)? ] $(, $($r:tt)*)?) => {
        $s.parse_dom = ::std::vec![ $( $crate::ParseRule::tag($t) ),* ];
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };
    (@node_fields $s:ident, attrs: { $($an:ident: $ad:expr),* $(,)? } $(, $($r:tt)*)?) => {
        $(
            $s.attrs.insert(
                ::std::string::String::from(stringify!($an)),
                $crate::AttrSpec { default: ::core::option::Option::Some($crate::AttrValue::from($ad)) },
            );
        )*
        $crate::schema!(@node_fields $s, $($($r)*)?);
    };

    // ---- mark list ----------------------------------------------------
    (@marks $b:expr,) => { $b };
    (@marks $b:expr, $name:ident { $($fields:tt)* } $(, $($rest:tt)*)?) => {{
        #[allow(unused_mut)]
        let mut __spec = $crate::MarkSpec::default();
        $crate::schema!(@mark_fields __spec, $($fields)*);
        let __b = $b.mark(stringify!($name), __spec);
        $crate::schema!(@marks __b, $($($rest)*)?)
    }};

    // ---- mark fields --------------------------------------------------
    (@mark_fields $s:ident,) => {};
    (@mark_fields $s:ident, group: $v:expr $(, $($r:tt)*)?) => {
        $s.group = ::core::option::Option::Some($v.into());
        $crate::schema!(@mark_fields $s, $($($r)*)?);
    };
    (@mark_fields $s:ident, inclusive: $v:expr $(, $($r:tt)*)?) => {
        $s.inclusive = $v;
        $crate::schema!(@mark_fields $s, $($($r)*)?);
    };
    (@mark_fields $s:ident, dom: $v:expr $(, $($r:tt)*)?) => {
        $s.to_dom = ::core::option::Option::Some(|_| $crate::DomSpec::element($v));
        $crate::schema!(@mark_fields $s, $($($r)*)?);
    };
    (@mark_fields $s:ident, to_dom: $v:expr $(, $($r:tt)*)?) => {
        $s.to_dom = ::core::option::Option::Some($v);
        $crate::schema!(@mark_fields $s, $($($r)*)?);
    };
    (@mark_fields $s:ident, parse: [ $($t:expr),* $(,)? ] $(, $($r:tt)*)?) => {
        $s.parse_dom = ::std::vec![ $( $crate::ParseRule::tag($t) ),* ];
        $crate::schema!(@mark_fields $s, $($($r)*)?);
    };
    (@mark_fields $s:ident, attrs: { $($an:ident: $ad:expr),* $(,)? } $(, $($r:tt)*)?) => {
        $(
            $s.attrs.insert(
                ::std::string::String::from(stringify!($an)),
                $crate::AttrSpec { default: ::core::option::Option::Some($crate::AttrValue::from($ad)) },
            );
        )*
        $crate::schema!(@mark_fields $s, $($($r)*)?);
    };
}
