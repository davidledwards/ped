//! Binds key sequences to editing operations.
//!
//! The recognized set of keys that can be used in the formation of sequences is
//! defined authoritatively in the map produced by [`init_key_map`](key::init_key_map),
//! and similarly, the recognized set of editing operations is defined in the map
//! produced by [`init_op_map`](op::init_op_map).

use crate::error::{Error, Result};
use crate::key::{self, Key, KeyMap};
use crate::op;
use crate::operation::Operation;
use std::collections::{HashMap, HashSet};

/// A mapping of [`Key`] sequences to editing functions.
pub struct Bindings {
    key_map: KeyMap,
    op_map: HashMap<&'static str, Operation>,
    bind_map: HashMap<Vec<Key>, String>,
    bind_prefixes: HashSet<Vec<Key>>,
    restricted_keys: HashSet<Vec<Key>>,
}

impl Bindings {
    /// Creates the default key bindings.
    ///
    /// The restricted keys are not enforced in this function, in contrast to
    /// [`bind()`](Self::bind), as initialization of default bindinds needs to
    /// bind these keys.
    pub fn new(bindings: &HashMap<String, String>) -> Result<Bindings> {
        let mut this = Bindings {
            key_map: key::init_key_map(),
            op_map: op::init_op_map(),
            bind_map: HashMap::new(),
            bind_prefixes: HashSet::new(),
            restricted_keys: Self::init_restricted_keys(),
        };

        for (key_seq, op) in bindings {
            this.bind_internal(key_seq, op, false)?;
        }
        Ok(this)
    }

    /// Binds the key sequence `key_seq` to the editing operation `op`, which will
    /// override an existing binding with an identical key sequence.
    ///
    /// Attempting to bind to any of the restricted key sequences will result in an
    /// error.
    pub fn bind(&mut self, key_seq: &str, op: &str) -> Result<()> {
        self.bind_internal(key_seq, op, true)
    }

    /// Internal binding function that prohibits binding to restricted key sequences
    /// when `strict` is `true`.
    fn bind_internal(&mut self, key_seq: &str, op: &str, strict: bool) -> Result<()> {
        self.to_keys(key_seq, strict).and_then(|keys| {
            self.op_map
                .get_key_value(op)
                .map(|(op, _)| {
                    self.bind_map.insert(keys.clone(), op.to_string());
                    for n in 1..keys.len() {
                        let prefix = &keys[0..n];
                        self.bind_prefixes.insert(prefix.to_vec());
                    }
                })
                .ok_or_else(|| Error::invalid_op(op))
        })
    }

    /// Converts the key sequence `key_seq` to a vector of [`Key']s.
    ///
    /// If `strict` is `true`, then the presence of a restricted key sequence will
    /// result in `Err`.
    fn to_keys(&self, key_seq: &str, strict: bool) -> Result<Vec<Key>> {
        // Preprocess key sequence by expanding occurrences of `M-<key>` into
        // `ESC` + `<key>`.
        let mut keys = key_seq.split(":").collect::<Vec<_>>();
        let mut i = 0;
        while i < keys.len() {
            if let Some(key) = keys[i].strip_prefix("M-") {
                keys[i] = key;
                keys.insert(i, "ESC");
            }
            i += 1;
        }

        // Check validity of key names and produce vector of keys.
        let keys = keys
            .iter()
            .map(|key| {
                self.key_map
                    .get(key)
                    .cloned()
                    .or_else(|| {
                        let mut chars = key.chars();
                        if let (Some(c), None) = (chars.next(), chars.next()) {
                            Some(Key::Char(c))
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| Error::invalid_key(key))
            })
            .collect::<Result<Vec<Key>>>();

        // Postprocess to ensure vector of keys is not restricted.
        if strict {
            keys.and_then(|keys| {
                if self.restricted_keys.contains(&keys) {
                    Err(Error::restricted_key(key_seq))
                } else {
                    Ok(keys)
                }
            })
        } else {
            keys
        }
    }

    /// Returns a reference to the current bindings.
    pub fn bindings(&self) -> &HashMap<Vec<Key>, String> {
        &self.bind_map
    }

    /// Returns the function pointer bound to `keys`, otherwise `None`.
    pub fn find(&self, keys: &Vec<Key>) -> Option<&Operation> {
        self.bind_map
            .get(keys)
            .and_then(|op| self.op_map.get(op as &str))
    }

    /// Returns a vector of keys bound to `op`, which may be empty.
    pub fn find_key(&self, op: &str) -> Vec<Vec<Key>> {
        self.bind_map
            .iter()
            .filter(|(_, o)| o as &str == op)
            .map(|(key, _)| key.to_owned())
            .collect()
    }

    /// Returns `true` if `keys` is a prefix of at least one key sequence bound to a
    /// function pointer.
    pub fn is_prefix(&self, keys: &Vec<Key>) -> bool {
        self.bind_prefixes.contains(keys)
    }

    /// Returns the set of restricted keys.
    fn init_restricted_keys() -> HashSet<Vec<Key>> {
        let mut keys = HashSet::new();
        for key_seq in Self::RESTRICTED_KEYS {
            keys.insert(key_seq.to_vec());
        }
        keys
    }

    /// A collection of key sequences that are restricted from being rebound.
    const RESTRICTED_KEYS: [&[Key]; 3] = [
        &[Key::Control(7)],  // C-g
        &[Key::Control(17)], // C-q
        &[Key::Control(27)], // C-[ (ESC)
    ];
}
