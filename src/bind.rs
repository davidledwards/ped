//! Key bindings.
use crate::error::{Error, Result};
use crate::key::{self, Key, KeyMap};
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

impl Bindings {
    /// Creates the default key bindings.
    pub fn new() -> Bindings {
        let mut this = Bindings {
            key_map: key::init_key_map(),
            op_map: op::init_op_map(),
            bind_map: BindMap::new(),
            bind_prefixes: Prefixes::new(),
        };

        for (key_seq, op) in Self::DEFAULT_BINDINGS {
            this.bind(key_seq, op).unwrap_or_else(|e| panic!("{e}"));
        }
        this
    }

    /// Binds the key sequence `key_seq` to the editing operation `op`.
    ///
    /// A successful bind will override an existing binding with an identical key
    /// sequence.
    ///
    /// # Errors
    ///
    /// Returns an [`Err`] if either of `key_seq` or `op` do not match the name of
    /// known keys or editing operations, respectively.
    pub fn bind(&mut self, key_seq: &str, op: &str) -> Result<()> {
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

    /// Converts the key sequence `key_seq` to a vector of [`Key']s.
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

    /// Default mapping of keys to editing operations.
    const DEFAULT_BINDINGS: [(&'static str, &'static str); 36] = [
        // Exit and cancellation.
        ("ctrl-q", "quit"),
        // Navigation.
        ("ctrl-b", "move-left"),
        ("left", "move-left"),
        ("ctrl-f", "move-right"),
        ("right", "move-right"),
        ("ctrl-p", "move-up"),
        ("up", "move-up"),
        ("ctrl-n", "move-down"),
        ("down", "move-down"),
        ("ctrl-[:p", "move-page-up"),
        ("page-up", "move-page-up"),
        ("ctrl-[:n", "move-page-down"),
        ("page-down", "move-page-down"),
        ("ctrl-a", "move-start-line"),
        ("home", "move-start-line"),
        ("ctrl-e", "move-end-line"),
        ("end", "move-end-line"),
        ("ctrl-home", "move-top"),
        ("ctrl-[:a", "move-top"),
        ("ctrl-end", "move-bottom"),
        ("ctrl-[:e", "move-bottom"),
        ("shift-ctrl-up", "scroll-up"),
        ("shift-ctrl-down", "scroll-down"),
        ("ctrl-l", "scroll-center"),
        // Insertion and deletion.
        ("ctrl-d", "remove-char-right"),
        ("ctrl-?", "remove-char-left"),
        ("ctrl-h", "remove-char-left"),
        ("ctrl-m", "insert-line"),
        // Windows (FIXME: temporary until we find proper home)
        ("ctrl-w:/", "open-window-top"),
        ("ctrl-w:\\", "open-window-bottom"),
        ("ctrl-w:[", "open-window-above"),
        ("ctrl-w:]", "open-window-below"),
        ("ctrl-w:k", "close-window"),
        ("ctrl-w:p", "prev-window"),
        ("ctrl-w:n", "next-window"),
        // Files.
        ("ctrl-o", "open-file"),
    ];
}
