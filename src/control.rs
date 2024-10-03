//! Main controller.
use crate::bind::BindingMap;
use crate::editor::EditorRef;
use crate::error::Result;
use crate::key::{Key, Keyboard};
use crate::op::{Action, ContinueFn};
use crate::session::Session;
use crate::workspace::Workspace;

use std::fmt;
use std::time::Instant;

/// The primary control point for coordinating user interaction and editing operations.
pub struct Controller {
    keyboard: Keyboard,
    bindings: BindingMap,
    session: Session,
    context: Context,
}

/// Execution context of [`Controller`] that manages state.
struct Context {
    /// An optional time of the last alert displayed to user or `None` if the alert has
    /// been cleared.
    last_alert: Option<Instant>,

    /// An optional continuation function that will process the next [`Key`].
    cont_fn: Option<Box<ContinueFn>>,

    /// A stack of keys representing a sequence of continuation function.
    ///
    /// In theory, an editing operation could return an infinite number of continuation
    /// functions, but in practice, there is usually one or possibly two continuations.
    /// A continuation would be used to represent a *collection* of like operations.
    key_stack: Vec<Key>,
}

impl Context {
    fn new() -> Context {
        Context {
            last_alert: None,
            cont_fn: None,
            key_stack: Vec::new(),
        }
    }
}

/// Wrapper used only for formatting [`Key`] sequences.
struct KeyStack<'a>(&'a Vec<Key>);

impl fmt::Display for KeyStack<'_> {
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
        bindings: BindingMap,
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

    pub fn run(&mut self) -> Result<()> {
        loop {
            let key = self.keyboard.read()?;
            if key == Key::None {
                // do background stuff here
            } else {
                let action = if let Some(mut cn_fn) = self.context.cont_fn.take() {
                    cn_fn(&mut self.session, &key)?
                } else {
                    match self.bindings.lookup(&key) {
                        Some(op_fn) => op_fn(&mut self.session, &key)?,
                        None => Action::UndefinedKey(key.clone()),
                    }
                };
                match action {
                    Action::Nothing => {
                        self.reset_alert();
                        self.context.key_stack.clear();
                    }
                    Action::Continue(cn_fn) => {
                        self.context.cont_fn = Some(cn_fn);
                        self.context.key_stack.push(key.clone());
                        let text = KeyStack(&self.context.key_stack).to_string();
                        self.set_alert(text.as_str());
                    }
                    Action::Alert(text) => {
                        self.set_alert(text.as_str());
                        self.context.key_stack.clear();
                    }
                    Action::UndefinedKey(key) => {
                        let mut key_stack = self.context.key_stack.clone();
                        self.context.key_stack.clear();
                        key_stack.push(key.clone());
                        let text = format!(
                            "{}: undefined key{}",
                            KeyStack(&key_stack),
                            if key_stack.len() == 1 { "" } else { "sequence" }
                        );
                        self.set_alert(text.as_str());
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
