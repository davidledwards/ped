//! Key bindings.
use crate::error::{Error, Result};
use crate::key::{self, Ctrl, Key, KeyMap, Shift};
use crate::op::{self, OpFn, OpMap};

use std::collections::{HashMap, HashSet};

/// Map of [`Key`] sequences to editing operations.
type BindMap = HashMap<Vec<Key>, &'static str>;

/// Set of [`Key`] sequence prefixes.
type Prefixes = HashSet<Vec<Key>>;

/// A mapping of [`Key`] sequences to editing functions.
pub struct Bindings {
    key_map: KeyMap,
    op_map: OpMap,
    bind_map: BindMap,
    bind_prefixes: Prefixes,
}

const OTHER_BINDINGS: [(&'static str, &'static str); 6] = [
    ("ctrl-w:/", "open-window-top"),
    ("ctrl-w:\\", "open-window-bottom"),
    ("ctrl-w:[", "open-window-above"),
    ("ctrl-w:]", "open-window-below"),
    ("ctrl-w:p", "prev-window"),
    ("ctrl-w:n", "next-window"),
];

impl Bindings {
    /// Creates the default key bindings.
    pub fn new() -> Bindings {
        let mut this = Bindings {
            key_map: key::init_key_map(),
            op_map: op::init_op_map(),
            bind_map: BindMap::new(),
            bind_prefixes: Prefixes::new(),
        };

        // Since default bindings are single keys, as opposed to key sequences,
        // process of adding prefixes can be skipped.
        for (key, op) in Self::DEFAULT_BINDINGS.iter() {
            if let Some((op, _)) = this.op_map.get_key_value(op as &str) {
                this.bind_map.insert(vec![key.clone()], op);
            } else {
                panic!("{op}: operation not found");
            }
        }

        for (key_seq, op) in OTHER_BINDINGS {
            this.bind(key_seq, op).unwrap_or_else(|e| panic!("{e}"));
        }
        this
    }

    fn to_keys(&self, names: &str) -> Result<Vec<Key>> {
        names
            .split(':')
            .map(|name| {
                self.key_map
                    .get(name)
                    .cloned()
                    .or_else(|| {
                        let mut chars = name.chars();
                        if let (Some(c), None) = (chars.next(), chars.next()) {
                            Some(Key::Char(c))
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| Error::invalid_key(name))
            })
            .collect()
    }

    fn bind(&mut self, key_seq: &str, op: &str) -> Result<()> {
        self.to_keys(key_seq).and_then(|keys| {
            self.op_map
                .get_key_value(op)
                .map(|(op, _)| {
                    self.bind_map.insert(keys.clone(), op);
                    for n in 1..keys.len() {
                        let prefix = &keys[0..n];
                        self.bind_prefixes.insert(prefix.to_vec());
                    }
                })
                .ok_or_else(|| Error::bind_op(op))
        })
    }

    /// Binds the collection of key sequences to editing operations specified in
    /// `bindings`.
    ///
    /// The mappings in `bindings` will override any existing bindings with identical
    /// key sequences. Any element with an empty vector of keys is quietly ignored.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if any entry in `bindings` does not match the name of an
    /// editing operation.
    pub fn bind_all(&mut self, bindings: &[(Vec<Key>, String)]) -> Result<()> {
        for (keys, op) in bindings {
            if keys.len() > 0 {
                if let Some((op, _)) = self.op_map.get_key_value(op as &str) {
                    self.bind_map.insert(keys.clone(), op);
                    for n in 1..keys.len() {
                        let prefix = &keys[0..n];
                        self.bind_prefixes.insert(prefix.to_vec());
                    }
                } else {
                    return Err(Error::bind_op(op));
                }
            }
        }
        Ok(())
    }

    /// Returns the function pointer bound to `keys`, otherwise `None`.
    pub fn find(&self, keys: &Vec<Key>) -> Option<&OpFn> {
        self.bind_map
            .get(keys)
            .and_then(|op| self.op_map.get(op as &str))
    }

    /// Returns `true` if `keys` is a prefix of at least one key sequence bound to a
    /// function pointer.
    pub fn is_prefix(&self, keys: &Vec<Key>) -> bool {
        self.bind_prefixes.contains(keys)
    }

    /// Default mapping of [`Key`]s to editing operations.
    const DEFAULT_BINDINGS: [(Key, &'static str); 25] = [
        (Key::Control(13), "insert-line"),
        (Key::Control(127), "delete-char-left"),
        (Key::Control(8), "delete-char-left"),
        (Key::Control(4), "delete-char-right"),
        (Key::Up(Shift::Off, Ctrl::Off), "move-up"),
        (Key::Control(16), "move-up"),
        (Key::Down(Shift::Off, Ctrl::Off), "move-down"),
        (Key::Control(14), "move-down"),
        (Key::Left(Shift::Off, Ctrl::Off), "move-left"),
        (Key::Control(2), "move-left"),
        (Key::Right(Shift::Off, Ctrl::Off), "move-right"),
        (Key::Control(6), "move-right"),
        (Key::PageUp(Shift::Off, Ctrl::Off), "move-page-up"),
        (Key::PageDown(Shift::Off, Ctrl::Off), "move-page-down"),
        (Key::Home(Shift::Off, Ctrl::On), "move-top"),
        (Key::End(Shift::Off, Ctrl::On), "move-bottom"),
        (Key::Up(Shift::On, Ctrl::On), "scroll-up"),
        (Key::Down(Shift::On, Ctrl::On), "scroll-down"),
        (Key::Home(Shift::Off, Ctrl::Off), "move-begin-line"),
        (Key::Control(1), "move-begin-line"),
        (Key::End(Shift::Off, Ctrl::Off), "move-end-line"),
        (Key::Control(5), "move-end-line"),
        (Key::Control(18), "redraw"),
        (Key::Control(12), "redraw-and-center"),
        (Key::Control(17), "quit"),
    ];
}
