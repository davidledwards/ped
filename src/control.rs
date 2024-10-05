//! Main controller.
use crate::bind::Bindings;
use crate::editor::EditorRef;
use crate::error::Result;
use crate::key::{Key, Keyboard};
use crate::op::Action;
use crate::session::Session;
use crate::workspace::Workspace;

use std::fmt;
use std::time::Instant;

/// The primary control point for coordinating user interaction and editing operations.
pub struct Controller {
    keyboard: Keyboard,
    bindings: Bindings,
    session: Session,
    context: Context,
}

/// Execution context of [`Controller`] that manages state.
struct Context {
    /// An optional time of the last alert displayed to user or `None` if the alert has
    /// been cleared.
    last_alert: Option<Instant>,

    /// A sequence of keys resulting from continuations.
    key_seq: Vec<Key>,
}

impl Context {
    fn new() -> Context {
        Context {
            last_alert: None,
            key_seq: Vec::new(),
        }
    }
}

/// Wrapper used only for formatting [`Key`] sequences.
struct KeySeq<'a>(&'a Vec<Key>);

impl fmt::Display for KeySeq<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key_seq = self
            .0
            .iter()
            .map(|key| key.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        write!(f, "{key_seq}")
    }
}

impl Controller {
    pub fn new(
        keyboard: Keyboard,
        bindings: Bindings,
        workspace: Workspace,
        editors: Vec<EditorRef>,
    ) -> Controller {
        let session = Session::new(workspace, editors);

        Controller {
            keyboard,
            bindings,
            session,
            context: Context::new(),
        }
    }

    fn is_char(&self, key: &Key) -> Option<char> {
        if self.context.key_seq.is_empty() {
            if let Key::Char(c) = key {
                Some(*c)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            let key = self.keyboard.read()?;
            if key == Key::None {
                // do background stuff here
            } else {
                let action = if let Some(c) = self.is_char(&key) {
                    self.session.active_editor().insert_char(c);
                    Action::Nothing
                } else {
                    self.context.key_seq.push(key.clone());
                    if let Some(op_fn) = self.bindings.find(&self.context.key_seq) {
                        op_fn(&mut self.session)?
                    } else if self.bindings.is_prefix(&self.context.key_seq) {
                        Action::Continue
                    } else {
                        Action::UndefinedKey
                    }
                };
                match action {
                    Action::Nothing => {
                        self.reset_alert();
                        self.context.key_seq.clear();
                    }
                    Action::Continue => {
                        let text = KeySeq(&self.context.key_seq).to_string();
                        self.set_alert(text.as_str());
                    }
                    Action::Alert(text) => {
                        self.set_alert(text.as_str());
                        self.context.key_seq.clear();
                    }
                    Action::UndefinedKey => {
                        let key_seq = &self.context.key_seq;
                        let text = format!(
                            "{}: undefined {}",
                            KeySeq(&key_seq),
                            if key_seq.len() == 1 {
                                "key"
                            } else {
                                "key sequence"
                            }
                        );
                        self.set_alert(text.as_str());
                        self.context.key_seq.clear();
                    }
                    Action::Quit => break,
                }
            }
        }
        Ok(())
    }

    fn set_alert(&mut self, text: &str) {
        self.session.workspace.alert(text);
        self.context.last_alert = Some(Instant::now());
    }

    fn reset_alert(&mut self) {
        if let Some(_) = self.context.last_alert.take() {
            self.session.workspace.alert("");
        }
    }
}
