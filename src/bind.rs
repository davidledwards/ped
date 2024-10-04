//! Key bindings.
use crate::error::{Error, Result};
use crate::key::{Ctrl, Key, Shift};
use crate::op::{self, OpFn};

use std::collections::{HashMap, HashSet};

/// Map of canonical editing operations to editing functions.
type OpMap = HashMap<&'static str, OpFn>;

/// Map of keys to canonical names.
type KeyMap = HashMap<Key, &'static str>;

/// Map of canonical key names to canonical editing operations.
type BindMap = HashMap<&'static str, &'static str>;

/// A binding of keys to editing operations.
///
/// Bindings are essentially a mapping of canonical key names to canonical editing
/// operations. Such associations are made at runtime, essentially allowing custom
/// bindings.
pub struct BindingMap {
    key_map: KeyMap,
    op_map: OpMap,
    bind_map: BindMap,
}

impl BindingMap {
    /// Constructs the default binding of keys.
    ///
    /// Unlike [`with_bindings`](Self::with_bindings), this function always succeeds.
    pub fn new() -> BindingMap {
        let bindings: Vec<(String, String)> = DEFAULT_BINDINGS
            .iter()
            .map(|(name, op)| (name.to_string(), op.to_string()))
            .collect();
        Self::with_bindings(&bindings).unwrap_or_else(|e| {
            // If this condition occurs, there is an invariant violation.
            panic!("{e}")
        })
    }

    /// Constructs a binding of keys using an array of canonical (_key-name_, _op-name_)
    /// pairs.
    ///
    /// Both _key-name_ and _op-name_ must match the value of an entry in [`KEY_MAPPINGS`] and
    /// and the key of an entry in [`OP_MAPPINGS`], respectively. Otherwise, construction of
    /// the map fails.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if any of the _key-name_ or _op-name_ references fail to match
    /// the predefined collection of canonical names.
    pub fn with_bindings(bindings: &[(String, String)]) -> Result<BindingMap> {
        let mut this = BindingMap {
            key_map: init_key_map(),
            op_map: init_op_map(),
            bind_map: BindMap::new(),
        };
        this.bind(bindings)?;
        Ok(this)
    }

    /// Return the function pointer bound to [`Key`], otherwise `None`.
    pub fn lookup(&self, key: &Key) -> Option<&OpFn> {
        self.map_key(key)
            .and_then(|name| self.bind_map.get(name))
            .and_then(|op| self.op_map.get(op as &str))
    }

    fn map_key(&self, key: &Key) -> Option<&'static str> {
        match key {
            Key::Char(_) => Some("char"),
            _ => self.key_map.get(key).map(|name| *name),
        }
    }

    fn bind(&mut self, bindings: &[(String, String)]) -> Result<()> {
        // Extract canonical key names so given bindings can be verified to exist
        // before actually trying to bind. A special "char" name is added since this needs to
        // be resolved, but cannot exist in predefined key mappings because all characters
        // map to this name.
        let mut key_names: HashSet<&'static str> = self.key_map.values().cloned().collect();
        key_names.insert("char");

        for (name, op) in bindings {
            if let Some(name) = key_names.get(name.as_str()) {
                if let Some((op, _)) = self.op_map.get_key_value(op.as_str()) {
                    self.bind_map.insert(name, op);
                } else {
                    return Err(Error::bind_op(op));
                }
            } else {
                return Err(Error::bind_key(name));
            }
        }
        Ok(())
    }
}

/// Default key bindings that associate canonical key names to canonical editing operations.
const DEFAULT_BINDINGS: [(&'static str, &'static str); 28] = [
    ("ctrl-[", "meta-key"),
    ("char", "insert-char"),
    ("ctrl-m", "insert-line"),
    ("ctrl-?", "delete-char-left"),
    ("ctrl-h", "delete-char-left"),
    ("ctrl-d", "delete-char-right"),
    ("up", "move-up"),
    ("ctrl-p", "move-up"),
    ("down", "move-down"),
    ("ctrl-n", "move-down"),
    ("left", "move-left"),
    ("ctrl-b", "move-left"),
    ("right", "move-right"),
    ("ctrl-f", "move-right"),
    ("page-up", "move-page-up"),
    ("page-down", "move-page-down"),
    ("ctrl-home", "move-top"),
    ("ctrl-end", "move-bottom"),
    ("shift-ctrl-up", "scroll-up"),
    ("shift-ctrl-down", "scroll-down"),
    ("home", "move-begin-line"),
    ("ctrl-a", "move-begin-line"),
    ("end", "move-end-line"),
    ("ctrl-e", "move-end-line"),
    ("ctrl-r", "redraw"),
    ("ctrl-l", "redraw-and-center"),
    ("ctrl-x", "quit"),
    ("ctrl-w", "window-key"),
];

/// Predefined key mappings that associate well known [`Key`]s with canonical names.
///
/// Canonical names are used for the runtime binding of keys to editing operations,
/// which themselves are named and well known.
///
/// Note that [`Key::Char`] is absent from these mappings because of the impracticality
/// of mapping all possible characters.
const KEY_MAPPINGS: [(Key, &'static str); 86] = [
    (Key::Control(0), "ctrl-@"),
    (Key::Control(1), "ctrl-a"),
    (Key::Control(2), "ctrl-b"),
    (Key::Control(3), "ctrl-c"),
    (Key::Control(4), "ctrl-d"),
    (Key::Control(5), "ctrl-e"),
    (Key::Control(6), "ctrl-f"),
    (Key::Control(7), "ctrl-g"),
    (Key::Control(8), "ctrl-h"),
    (Key::Control(9), "ctrl-i"),
    (Key::Control(10), "ctrl-j"),
    (Key::Control(11), "ctrl-k"),
    (Key::Control(12), "ctrl-l"),
    (Key::Control(13), "ctrl-m"),
    (Key::Control(14), "ctrl-n"),
    (Key::Control(15), "ctrl-o"),
    (Key::Control(16), "ctrl-p"),
    (Key::Control(17), "ctrl-q"),
    (Key::Control(18), "ctrl-r"),
    (Key::Control(19), "ctrl-s"),
    (Key::Control(20), "ctrl-t"),
    (Key::Control(21), "ctrl-u"),
    (Key::Control(22), "ctrl-v"),
    (Key::Control(23), "ctrl-w"),
    (Key::Control(24), "ctrl-x"),
    (Key::Control(25), "ctrl-y"),
    (Key::Control(26), "ctrl-z"),
    (Key::Control(27), "ctrl-["),
    (Key::Control(28), "ctrl-\\"),
    (Key::Control(29), "ctrl-]"),
    (Key::Control(30), "ctrl-^"),
    (Key::Control(31), "ctrl-_"),
    (Key::Control(127), "ctrl-?"),
    (Key::ShiftTab, "shift-tab"),
    (Key::Up(Shift::Off, Ctrl::Off), "up"),
    (Key::Up(Shift::On, Ctrl::Off), "shift-up"),
    (Key::Up(Shift::Off, Ctrl::On), "ctrl-up"),
    (Key::Up(Shift::On, Ctrl::On), "shift-ctrl-up"),
    (Key::Down(Shift::Off, Ctrl::Off), "down"),
    (Key::Down(Shift::On, Ctrl::Off), "shift-down"),
    (Key::Down(Shift::Off, Ctrl::On), "ctrl-down"),
    (Key::Down(Shift::On, Ctrl::On), "shift-ctrl-down"),
    (Key::Left(Shift::Off, Ctrl::Off), "left"),
    (Key::Left(Shift::On, Ctrl::Off), "shift-left"),
    (Key::Left(Shift::Off, Ctrl::On), "ctrl-left"),
    (Key::Left(Shift::On, Ctrl::On), "shift-ctrl-left"),
    (Key::Right(Shift::Off, Ctrl::Off), "right"),
    (Key::Right(Shift::On, Ctrl::Off), "shift-right"),
    (Key::Right(Shift::Off, Ctrl::On), "ctrl-right"),
    (Key::Right(Shift::On, Ctrl::On), "shift-ctrl-right"),
    (Key::Home(Shift::Off, Ctrl::Off), "home"),
    (Key::Home(Shift::On, Ctrl::Off), "shift-home"),
    (Key::Home(Shift::Off, Ctrl::On), "ctrl-home"),
    (Key::Home(Shift::On, Ctrl::On), "shift-ctrl-home"),
    (Key::End(Shift::Off, Ctrl::Off), "end"),
    (Key::End(Shift::On, Ctrl::Off), "shift-end"),
    (Key::End(Shift::Off, Ctrl::On), "ctrl-end"),
    (Key::End(Shift::On, Ctrl::On), "shift-ctrl-end"),
    (Key::PageUp(Shift::Off, Ctrl::Off), "page-up"),
    (Key::PageUp(Shift::On, Ctrl::Off), "shift-page-up"),
    (Key::PageUp(Shift::Off, Ctrl::On), "ctrl-page-up"),
    (Key::PageUp(Shift::On, Ctrl::On), "shift-ctrl-page-up"),
    (Key::PageDown(Shift::Off, Ctrl::Off), "page-down"),
    (Key::PageDown(Shift::On, Ctrl::Off), "shift-page-down"),
    (Key::PageDown(Shift::Off, Ctrl::On), "ctrl-page-down"),
    (Key::PageDown(Shift::On, Ctrl::On), "shift-ctrl-page-down"),
    (Key::Function(1), "fn-1"),
    (Key::Function(2), "fn-2"),
    (Key::Function(3), "fn-3"),
    (Key::Function(4), "fn-4"),
    (Key::Function(5), "fn-5"),
    (Key::Function(6), "fn-6"),
    (Key::Function(7), "fn-7"),
    (Key::Function(8), "fn-8"),
    (Key::Function(9), "fn-9"),
    (Key::Function(10), "fn-10"),
    (Key::Function(11), "fn-11"),
    (Key::Function(12), "fn-12"),
    (Key::Function(13), "fn-13"),
    (Key::Function(14), "fn-14"),
    (Key::Function(15), "fn-15"),
    (Key::Function(16), "fn-16"),
    (Key::Function(17), "fn-17"),
    (Key::Function(18), "fn-18"),
    (Key::Function(19), "fn-19"),
    (Key::Function(20), "fn-20"),
];

fn init_key_map() -> KeyMap {
    let mut key_map = KeyMap::new();
    for (key, name) in KEY_MAPPINGS {
        key_map.insert(key, name);
    }
    key_map
}

/// Predefined operation mappings that associate the canonical names of well known
/// editing operations with function pointers that carry out those operations.
///
/// Canonical names are used for the runtime binding of keys to editing operations,
/// which themselves are named and well known.
const OP_MAPPINGS: [(&'static str, OpFn); 21] = [
    ("meta-key", op::meta_key),
    ("insert-char", op::insert_char),
    ("insert-line", op::insert_line),
    ("delete-char-left", op::delete_char_left),
    ("delete-char-right", op::delete_char_right),
    ("move-up", op::move_up),
    ("move-down", op::move_down),
    ("move-left", op::move_left),
    ("move-right", op::move_right),
    ("move-page-up", op::move_page_up),
    ("move-page-down", op::move_page_down),
    ("move-top", op::move_top),
    ("move-bottom", op::move_bottom),
    ("scroll-up", op::scroll_up),
    ("scroll-down", op::scroll_down),
    ("move-begin-line", op::move_begin_line),
    ("move-end-line", op::move_end_line),
    ("redraw", op::redraw),
    ("redraw-and-center", op::redraw_and_center),
    ("quit", op::quit),
    ("window-key", op::window_key),
];

fn init_op_map() -> OpMap {
    let mut op_map = OpMap::new();
    for (op, edit) in OP_MAPPINGS {
        op_map.insert(op, edit);
    }
    op_map
}
