//! Key bindings.
use crate::error::{Error, Result};
use crate::key::{self, Ctrl, Key, Shift};
use crate::op::{self, OpFn, OpMap};

use std::collections::{HashMap, HashSet};

/// Map of [`Key`] sequences to editing operations.
type BindMap = HashMap<Vec<Key>, &'static str>;

/// Set of [`Key`] sequence prefixes.
type PrefixSet = HashSet<Vec<Key>>;

/// A mapping of [`Key`] sequences to editing functions.
pub struct Bindings {
    op_map: OpMap,
    bind_map: BindMap,
    bind_prefixes: PrefixSet,
}

impl Bindings {
    /// Creates the default key bindings.
    pub fn new() -> Bindings {
        let mut this = Bindings {
            op_map: op::init_op_map(),
            bind_map: BindMap::new(),
            bind_prefixes: PrefixSet::new(),
        };

        // Since default bindings are single keys, as opposed to key sequences,
        // adding bind prefixes can be skipped.
        for (key, op) in Self::DEFAULT_BINDINGS.iter() {
            if let Some((op, _)) = this.op_map.get_key_value(op as &str) {
                this.bind_map.insert(vec![key.clone()], op);
            } else {
                panic!("{op}: operation not found");
            }
        }

        // FIXME: hack a few key sequences for testing
        let key_map = key::init_key_map();
        let ext_bindings = [(
            vec![
                key_map.get("ctrl-x").unwrap().clone(),
                key_map.get("ctrl-w").unwrap().clone(),
                Key::Char('/'),
            ],
            "open-window-top".to_string(),
        )];
        let _ = this.bind(&ext_bindings);

        this
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
    pub fn bind(&mut self, bindings: &[(Vec<Key>, String)]) -> Result<()> {
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
