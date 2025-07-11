//! A clipboard that manages access to both local and global instances.
//!
//! A _local_ clipboard is scoped in the context of `ped`, whereas a _global_
//! clipboard is provided by the OS.

use arboard::Clipboard as GlobalClipboard;

pub struct Clipboard {
    local: Option<Vec<char>>,
}

pub enum Scope {
    Local,
    Global,
}

impl Clipboard {
    pub fn new() -> Clipboard {
        Clipboard { local: None }
    }

    /// Adds `text` to the clipboard specified by `scope`.
    pub fn set_text(&mut self, text: Vec<char>, scope: Scope) {
        match scope {
            Scope::Local => self.local = Some(text),
            Scope::Global => {
                // OS-specific clipboards behave in different ways, so recommendation
                // is to create new instance prior to each access. Note that operations
                // could fail, possibly because of OS limitations or nuances, so
                // quietly ignore.
                GlobalClipboard::new()
                    .and_then(|mut clip| clip.set_text(text.iter().collect::<String>()))
                    .unwrap_or(())
            }
        }
    }

    /// Returns optional text from the clipboard specified by `scope`.
    pub fn get_text(&self, scope: Scope) -> Option<Vec<char>> {
        match scope {
            Scope::Local => self.local.clone(),
            Scope::Global => {
                // OS-specific clipboards behave in different ways, so recommendation
                // is to create new instance prior to each access. Note that operations
                // could fail, possibly because of OS limitations or nuances, so
                // quietly ignore and treat as though text does not exist on clipboard.
                GlobalClipboard::new()
                    .and_then(|mut clip| clip.get_text())
                    .map(|text| text.chars().collect::<Vec<_>>())
                    .ok()
            }
        }
    }
}
