//! Main controller.
use crate::bind::Bindings;
use crate::editor::EditorRef;
use crate::env::Environment;
use crate::error::Result;
use crate::key::{Key, Keyboard};
use crate::op::Action;
use crate::term;
use crate::workspace::{Workspace, WorkspaceRef};

use std::fmt;
use std::time::Instant;

/// The primary control point for coordinating user interaction and editing operations.
pub struct Controller {
    keyboard: Keyboard,
    bindings: Bindings,
    workspace: WorkspaceRef,
    env: Environment,
    context: Context,
}

/// Execution context of [`Controller`] that manages state.
struct Context {
    /// An optional time capturing the last terminal size change event.
    term_changed: Option<Instant>,

    /// An optional time of the last alert displayed to user or `None` if the alert has
    /// been cleared.
    last_alert: Option<Instant>,

    /// A sequence of keys resulting from continuations.
    key_seq: Vec<Key>,
}

impl Context {
    fn new() -> Context {
        Context {
            term_changed: None,
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
    /// Number of milliseconds controller waits before resizing workspace after it notices a
    /// change.
    const TERM_CHANGE_DELAY: u128 = 100;

    pub fn new(
        keyboard: Keyboard,
        bindings: Bindings,
        workspace: Workspace,
        editors: Vec<EditorRef>,
    ) -> Controller {
        let workspace = workspace.to_ref();
        let env = Environment::new(workspace.clone(), editors);

        Controller {
            keyboard,
            bindings,
            workspace,
            env,
            context: Context::new(),
        }
    }

    /// Runs the main processing loop.
    ///
    /// This loop orchestrates the entire editing experience, reading sequences of
    /// [keys](Key) and calling their corresponding editing functions until instructed to
    /// quit.
    pub fn run(&mut self) -> Result<()> {
        loop {
            let key = self.keyboard.read()?;
            if key == Key::None {
                // Detect change in terminal size and resize workspace, but not immediately.
                // In practice, a rapid series of change events could be detected because
                // human movement is significantly slower.
                self.context.term_changed = if term::size_changed() {
                    // Restart clock when change is detected.
                    Some(Instant::now())
                } else if let Some(time) = self.context.term_changed.take() {
                    if time.elapsed().as_millis() > Self::TERM_CHANGE_DELAY {
                        // Resize once delay period expires.
                        self.env.resize();
                        None
                    } else {
                        // Keep waiting.
                        Some(time)
                    }
                } else {
                    None
                };
            } else if key == Key::Control(7) {
                self.clear_keys();
                self.clear_alert();
            } else {
                if let Some(c) = self.possible_char(&key) {
                    // Inserting text is statistically most prevalent scenario, so this
                    // short circuits detection and bypasses normal indirection of key
                    // binding.
                    self.env.active_editor().insert_char(c);
                    self.clear_alert();
                } else {
                    self.context.key_seq.push(key.clone());
                    if let Some(op_fn) = self.bindings.find(&self.context.key_seq) {
                        match op_fn(&mut self.env)? {
                            Some(Action::Quit) => break,
                            Some(Action::Alert(text)) => {
                                self.set_alert(text.as_str());
                            }
                            None => {
                                self.clear_alert();
                            }
                        }
                        self.clear_keys();
                    } else if self.bindings.is_prefix(&self.context.key_seq) {
                        // Current keys form a prefix of at least one sequence bound to an
                        // editing function.
                        self.show_keys();
                    } else {
                        // Current keys are not bound to an editing function, nor do they
                        // form a prefix.
                        self.show_undefined_keys();
                        self.clear_keys();
                    }
                }
            }
        }
        Ok(())
    }

    /// An efficient means of detecting the very common case of a single character,
    /// allowing the controller to optimize its handling.
    fn possible_char(&self, key: &Key) -> Option<char> {
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

    fn show_keys(&mut self) {
        let text = KeySeq(&self.context.key_seq).to_string();
        self.set_alert(text.as_str());
    }

    fn show_undefined_keys(&mut self) {
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
    }

    fn clear_keys(&mut self) {
        self.context.key_seq.clear();
    }

    fn set_alert(&mut self, text: &str) {
        self.workspace.borrow_mut().set_alert(text);
        self.context.last_alert = Some(Instant::now());
        self.env.active_editor().show_cursor();
    }

    fn clear_alert(&mut self) {
        if let Some(_) = self.context.last_alert.take() {
            self.workspace.borrow_mut().clear_alert();
            self.env.active_editor().show_cursor();
        }
    }
}
