//! Key mapping.

use crate::editor::Editor;
use crate::error::Result;
use crate::key::{Key, Modifier};

use std::collections::HashMap;

pub struct KeyDescrip {
    name: &'static str,
    key: Key,
}

impl KeyDescrip {
    const fn new(name: &'static str, key: Key) -> KeyDescrip {
        KeyDescrip { name, key }
    }
}

type KeyMap = HashMap<Key, Box<dyn Fn(&mut Editor) -> Result<()>>>;

pub struct KeyBindings {
    key_map: KeyMap,
}

static KEYS: [KeyDescrip; 1] = [KeyDescrip::new("ctrl-a", Key::Control(1))];

impl KeyBindings {
    pub fn new() -> KeyBindings {
        let mut key_map: KeyMap = HashMap::new();
        KeyBindings { key_map }
    }
}
