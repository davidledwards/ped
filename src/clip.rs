//! A clipboard that manages access to both local and global instances.
//!
//! A _local_ clipboard is scoped in the context of `ped`, whereas a _global_
//! clipboard is provided by the OS.

use arboard::Clipboard as GlobalClipboard;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Clipboard {
    local: Option<Vec<char>>,
}

pub type ClipboardRef = Rc<RefCell<Clipboard>>;

pub enum Scope {
    Local,
    Global,
}

impl Clipboard {
    pub fn new() -> Clipboard {
        Clipboard { local: None }
    }

    /// Turns the clipboard into a [`ClipboardRef`].
    pub fn into_ref(self) -> ClipboardRef {
        Rc::new(RefCell::new(self))
    }

    /// Adds `text` to the clipboard specified by `scope`.
    pub fn set_text(&mut self, text: Vec<char>, scope: Scope) {
        match scope {
            Scope::Local => self.local = Some(text),
            Scope::Global => GlobalClipboard::new()
                .and_then(|mut clip| clip.set_text(text.iter().collect::<String>()))
                .unwrap_or(()),
        }
    }

    /// Returns optional text from the clipboard specified by `scope`.
    pub fn get_text(&self, scope: Scope) -> Option<Vec<char>> {
        match scope {
            Scope::Local => self.local.clone(),
            Scope::Global => GlobalClipboard::new()
                .and_then(|mut clip| clip.get_text())
                .map(|text| text.chars().collect::<Vec<_>>())
                .ok(),
        }
    }
}
