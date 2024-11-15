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
    const DEFAULT_BINDINGS: [(&'static str, &'static str); 60] = [
        // --- exit and cancellation ---
        ("ctrl-q", "quit"),
        // --- navigation and selection ---
        ("ctrl-b", "move-left"),
        ("left", "move-left"),
        ("shift-left", "move-left-select"),
        ("ctrl-f", "move-right"),
        ("right", "move-right"),
        ("shift-right", "move-right-select"),
        ("ctrl-p", "move-up"),
        ("up", "move-up"),
        ("shift-up", "move-up-select"),
        ("ctrl-n", "move-down"),
        ("down", "move-down"),
        ("shift-down", "move-down-select"),
        ("ctrl-[:p", "move-up-page"),
        ("page-up", "move-up-page"),
        ("shift-page-up", "move-up-page-select"),
        ("ctrl-[:n", "move-down-page"),
        ("page-down", "move-down-page"),
        ("shift-page-down", "move-down-page-select"),
        ("ctrl-a", "move-start"),
        ("home", "move-start"),
        ("shift-home", "move-start-select"),
        ("ctrl-e", "move-end"),
        ("end", "move-end"),
        ("shift-end", "move-end-select"),
        ("ctrl-home", "move-top"),
        ("ctrl-[:a", "move-top"),
        ("shift-ctrl-home", "move-top-select"),
        ("ctrl-end", "move-bottom"),
        ("ctrl-[:e", "move-bottom"),
        ("shift-ctrl-end", "move-bottom-select"),
        ("shift-ctrl-up", "scroll-up"),
        ("shift-ctrl-down", "scroll-down"),
        ("ctrl-l", "scroll-center"),
        ("ctrl-@", "set-mark"),
        ("ctrl-_", "goto-line"),
        // --- insertion and removal ---
        ("ctrl-m", "insert-line"),
        ("ctrl-d", "remove-right"),
        ("ctrl-?", "remove-left"),
        ("ctrl-h", "remove-left"),
        ("ctrl-j", "remove-start"),
        ("ctrl-k", "remove-end"),
        // --- selection actions ---
        ("ctrl-c", "copy"),
        ("ctrl-v", "paste"),
        ("ctrl-x", "cut"),
        // --- file handling ---
        ("ctrl-o", "open-file"),
        ("ctrl-[:o:t", "open-file-top"),
        ("ctrl-[:o:b", "open-file-bottom"),
        ("ctrl-[:o:p", "open-file-above"),
        ("ctrl-[:o:n", "open-file-below"),
        ("ctrl-s", "save-file"),
        ("ctrl-[:s", "save-file-as"),
        // --- window handling ---
        ("ctrl-w", "kill-window"),
        ("ctrl-[:w:w", "close-window"),
        ("ctrl-[:w:t", "top-window"),
        ("ctrl-[:w:b", "bottom-window"),
        ("ctrl-[:w:p", "prev-window"),
        ("ctrl-[:,", "prev-window"),
        ("ctrl-[:w:n", "next-window"),
        ("ctrl-[:.", "next-window"),
    ];
}
