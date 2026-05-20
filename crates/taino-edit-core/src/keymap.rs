//! [`Keymap`] — bind canonical key strings to [`Command`]s, with
//! cross-platform `Mod` handling (Ctrl on Windows/Linux, Cmd/Meta on macOS).
//!
//! `core` is headless, so a [`KeyPress`] is a platform-neutral description of
//! a key event; framework adapters translate their native events into it.

use std::collections::HashMap;

use crate::commands::{
    caret_left, caret_line_end, caret_line_start, caret_right, chain, delete_backward,
    delete_forward, delete_selection, join_backward, join_forward, select_all, split_block,
    Command, Dispatch,
};
use crate::state::EditorState;

/// A platform-neutral key event.
#[derive(Debug, Clone)]
pub struct KeyPress {
    /// The key name: a single character (`"b"`) or a named key
    /// (`"Enter"`, `"Backspace"`, `"ArrowLeft"`, `"Home"`).
    pub key: String,
    /// Control held.
    pub ctrl: bool,
    /// Alt/Option held.
    pub alt: bool,
    /// Shift held.
    pub shift: bool,
    /// Meta/Cmd held.
    pub meta: bool,
}

impl KeyPress {
    /// A bare key with no modifiers.
    pub fn key(name: &str) -> Self {
        KeyPress {
            key: name.to_string(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        }
    }

    /// Builder: set Ctrl.
    pub fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }
    /// Builder: set Alt.
    pub fn alt(mut self) -> Self {
        self.alt = true;
        self
    }
    /// Builder: set Shift.
    pub fn shift(mut self) -> Self {
        self.shift = true;
        self
    }
    /// Builder: set Meta/Cmd.
    pub fn meta(mut self) -> Self {
        self.meta = true;
        self
    }

    fn canonical(&self) -> String {
        let mut s = String::new();
        if self.alt {
            s.push_str("Alt-");
        }
        if self.ctrl {
            s.push_str("Ctrl-");
        }
        if self.meta {
            s.push_str("Meta-");
        }
        if self.shift {
            s.push_str("Shift-");
        }
        s.push_str(&self.key);
        s
    }
}

/// A set of key→command bindings for one platform.
pub struct Keymap {
    mac: bool,
    bindings: HashMap<String, Command>,
}

impl Keymap {
    /// Build a keymap. `mac` selects whether `Mod` means Cmd/Meta (macOS) or
    /// Ctrl. Binding strings use `-`-separated modifiers, e.g.
    /// `"Mod-b"`, `"Mod-Shift-z"`, `"Enter"`.
    pub fn new(mac: bool, bindings: Vec<(&str, Command)>) -> Self {
        let mut map = HashMap::new();
        let mut km = Keymap {
            mac,
            bindings: HashMap::new(),
        };
        for (spec, cmd) in bindings {
            map.insert(km.normalize_spec(spec), cmd);
        }
        km.bindings = map;
        km
    }

    fn normalize_spec(&self, spec: &str) -> String {
        let parts: Vec<&str> = spec.split('-').collect();
        let (mods, key) = parts.split_at(parts.len() - 1);
        let (mut alt, mut ctrl, mut shift, mut meta) = (false, false, false, false);
        for m in mods {
            match *m {
                "Mod" => {
                    if self.mac {
                        meta = true;
                    } else {
                        ctrl = true;
                    }
                }
                "Cmd" | "Meta" => meta = true,
                "Ctrl" | "Control" => ctrl = true,
                "Alt" | "Option" => alt = true,
                "Shift" => shift = true,
                other => panic!("unknown key modifier `{other}`"),
            }
        }
        KeyPress {
            key: key[0].to_string(),
            ctrl,
            alt,
            shift,
            meta,
        }
        .canonical()
    }

    /// Handle a key press. Returns whether a binding matched (and ran, if a
    /// dispatch was given and the command applied).
    pub fn handle(
        &self,
        state: &EditorState,
        press: &KeyPress,
        dispatch: Option<&mut Dispatch<'_>>,
    ) -> bool {
        match self.bindings.get(&press.canonical()) {
            Some(cmd) => cmd(state, dispatch),
            None => false,
        }
    }

    /// Add or replace a binding by its `Mod-`-using key spec (e.g.
    /// `"Mod-b"`). Extensions use this to inject bindings on top of
    /// [`base_keymap`].
    pub fn add(&mut self, spec: &str, command: Command) {
        let canonical = self.normalize_spec(spec);
        self.bindings.insert(canonical, command);
    }

    /// Number of bindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether the keymap has no bindings.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

/// The baseline keymap every editor wants: Enter (split), Backspace/Delete
/// (selection → block-join → char), `Mod-a` (select all), and caret motion
/// (arrows, Home/End).
pub fn base_keymap(mac: bool) -> Keymap {
    let bindings: Vec<(&str, Command)> = vec![
        ("Enter", Box::new(split_block)),
        (
            "Backspace",
            chain(vec![
                Box::new(delete_selection),
                Box::new(join_backward),
                Box::new(delete_backward),
            ]),
        ),
        (
            "Delete",
            chain(vec![
                Box::new(delete_selection),
                Box::new(join_forward),
                Box::new(delete_forward),
            ]),
        ),
        ("Mod-a", Box::new(select_all)),
        ("ArrowLeft", Box::new(caret_left)),
        ("ArrowRight", Box::new(caret_right)),
        ("Home", Box::new(caret_line_start)),
        ("End", Box::new(caret_line_end)),
    ];
    Keymap::new(mac, bindings)
}
