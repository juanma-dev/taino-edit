//! `taino-edit-extensions` — the v0.1 extension set for taino-edit.
//!
//! Each extension is a small unit-struct module exposing:
//!
//! * [`Extension::schema_additions`] — node and mark types the extension
//!   contributes to the schema.
//! * [`Extension::keymap_entries`] — `(key, command)` pairs to splice on top
//!   of [`taino_edit_core::base_keymap`].
//!
//! Compose them with [`build_schema_with`] (starting from a builder that
//! declares the universal `doc`/`text` primitives) and
//! [`build_keymap_with`].

#![deny(unsafe_code)]
#![forbid(unstable_features)]
#![warn(missing_docs, rust_2018_idioms)]

use taino_edit_core::{
    base_keymap, Command, Keymap, MarkSpec, NodeSpec, Schema, SchemaBuilder, SchemaError,
};

pub mod align;
pub mod blockquote;
pub mod bold;
pub mod heading;
pub mod history;
pub mod image;
pub mod italic;
pub mod link;
pub mod paragraph;
pub mod transform_case;

pub use align::{align_center, align_justify, align_left, align_right, Align};
pub use blockquote::Blockquote;
pub use bold::Bold;
pub use heading::Heading;
pub use history::{redo_command, undo_command, History};
pub use image::{insert_image, Image};
pub use italic::Italic;
pub use link::{remove_link, set_link, Link};
pub use paragraph::Paragraph;
pub use transform_case::{to_lowercase, to_uppercase, TransformCase};

/// Node and mark types an extension contributes to the schema.
#[derive(Default)]
pub struct SchemaAdditions {
    /// `(name, spec)` pairs to register as node types.
    pub nodes: Vec<(String, NodeSpec)>,
    /// `(name, spec)` pairs to register as mark types.
    pub marks: Vec<(String, MarkSpec)>,
}

/// A composable editor extension. Each unit-struct extension (`Bold`,
/// `Italic`, …) implements this trait so they can be aggregated by
/// [`build_schema_with`] and [`build_keymap_with`].
pub trait Extension {
    /// A short human-readable identifier (for diagnostics / debug).
    fn name(&self) -> &str;

    /// Node/mark types this extension contributes to the schema.
    fn schema_additions(&self) -> SchemaAdditions {
        SchemaAdditions::default()
    }

    /// `(key_spec, command)` pairs to add on top of the base keymap. The
    /// schema is provided so the extension can resolve its own node/mark
    /// types into the commands it ships.
    fn keymap_entries(&self, _schema: &Schema) -> Vec<(String, Command)> {
        Vec::new()
    }
}

/// Aggregate every extension's node/mark additions on top of an existing
/// [`SchemaBuilder`] (which should already declare the universal
/// `doc`/`text` primitives), then build the schema.
pub fn build_schema_with(
    mut sb: SchemaBuilder,
    extensions: &[&dyn Extension],
    top_node: &str,
) -> Result<Schema, SchemaError> {
    for ext in extensions {
        let adds = ext.schema_additions();
        for (name, spec) in adds.nodes {
            sb = sb.node(&name, spec);
        }
        for (name, spec) in adds.marks {
            sb = sb.mark(&name, spec);
        }
    }
    sb.top_node(top_node).build()
}

/// Start from [`base_keymap`] for the platform and splice in every
/// extension's bindings. Later bindings override earlier ones.
pub fn build_keymap_with(extensions: &[&dyn Extension], schema: &Schema, mac: bool) -> Keymap {
    let mut km = base_keymap(mac);
    for ext in extensions {
        for (key, cmd) in ext.keymap_entries(schema) {
            km.add(&key, cmd);
        }
    }
    km
}
