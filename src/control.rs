//! Main controller.
use crate::bind::Bindings;
use crate::echo::Echo;
use crate::editor::EditorRef;
use crate::env::Environment;
use crate::error::Result;
use crate::input::{Directive, InputEditor};
use crate::key::{Key, Keyboard, CTRL_G};
use crate::op::{Action, AnswerFn};
use crate::term;
use crate::workspace::Workspace;

use std::fmt;
use std::time::Instant;

/// The primary control point for coordinating user interaction and editing operations.
pub struct Controller {
    /// A keyboard for reading [keys](Key).
    keyboard: Keyboard,

    /// The key sequences bound to editing functions.
    bindings: Bindings,

    /// The editing environment made accessible to editing functions.
    env: Environment,

    /// A sequence of keys resulting from continuations.
    key_seq: Vec<Key>,

    /// A means of echoing arbitrary text.
    echo: Echo,

    /// An optional time of the last echo displayed or `None` if the echo has been cleared.
    last_echo: Option<Instant>,

    /// A means of soliciting input.
    input: InputEditor,

    /// An optional question solicited by an editing function or `None` otherwise.
    question: Option<Box<AnswerFn>>,

    /// An optional time capturing the last terminal size change event.
    term_changed: Option<Instant>,
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
        let echo = Echo::new(workspace.clone());
        let input = InputEditor::new(workspace.clone());

        Controller {
            keyboard,
            bindings,
            env,
            key_seq: Vec::new(),
            echo,
            last_echo: None,
            input,
            question: None,
            term_changed: None,
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
                self.term_changed = if term::size_changed() {
                    // Restart clock when change is detected.
                    Some(Instant::now())
                } else if let Some(time) = self.term_changed.take() {
                    if time.elapsed().as_millis() > Self::TERM_CHANGE_DELAY {
                        // Resize once delay period expires.
                        self.env.resize();
                        self.resize_echo();
                        self.resize_question();
                        None
                    } else {
                        // Keep waiting.
                        Some(time)
                    }
                } else {
                    None
                };
            } else {
                if let Some(answer_fn) = self.question.as_mut() {
                    let action = if key == CTRL_G {
                        let action = answer_fn(&mut self.env, None)?;
                        self.clear_question();
                        action
                    } else {
                        match self.input.process_key(&key) {
                            Directive::Continue => None,
                            Directive::Accept => {
                                let answer = self.input.buffer();
                                let action = answer_fn(&mut self.env, Some(&answer))?;
                                self.clear_question();
                                action
                            }
                            Directive::Cancel => {
                                self.clear_question();
                                None
                            }
                        }
                    };
                    match action {
                        Some(Action::Quit) => break,
                        Some(Action::Alert(text)) => {
                            self.set_echo(text.as_str());
                        }
                        Some(Action::Question(prompt, answer_fn)) => {
                            self.clear_echo();
                            self.set_question(&prompt, answer_fn);
                        }
                        None => (),
                    }
                } else {
                    if let Some(c) = self.possible_char(&key) {
                        // Inserting text is statistically most prevalent scenario, so this
                        // short circuits detection and bypasses normal indirection of key
                        // binding.
                        self.env.active_editor().insert_char(c);
                        self.clear_echo();
                    } else if key == CTRL_G {
                        self.clear_keys();
                        self.clear_echo();
                    } else {
                        self.key_seq.push(key.clone());
                        if let Some(op_fn) = self.bindings.find(&self.key_seq) {
                            match op_fn(&mut self.env)? {
                                Some(Action::Quit) => break,
                                Some(Action::Alert(text)) => {
                                    self.set_echo(text.as_str());
                                }
                                Some(Action::Question(prompt, answer_fn)) => {
                                    self.clear_echo();
                                    self.set_question(&prompt, answer_fn);
                                }
                                None => {
                                    self.clear_echo();
                                }
                            }
                            self.clear_keys();
                        } else if self.bindings.is_prefix(&self.key_seq) {
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
        }
        Ok(())
    }

    /// An efficient means of detecting the very common case of a single character,
    /// allowing the controller to optimize its handling.
    fn possible_char(&self, key: &Key) -> Option<char> {
        if self.key_seq.is_empty() {
            if let Key::Char(c) = key {
                Some(*c)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn clear_keys(&mut self) {
        self.key_seq.clear();
    }

    fn show_keys(&mut self) {
        let text = KeySeq(&self.key_seq).to_string();
        self.set_echo(text.as_str());
    }

    fn show_undefined_keys(&mut self) {
        let key_seq = &self.key_seq;
        let text = format!(
            "{}: undefined {}",
            KeySeq(&key_seq),
            if key_seq.len() == 1 {
                "key"
            } else {
                "key sequence"
            }
        );
        self.set_echo(text.as_str());
    }

    fn set_echo(&mut self, text: &str) {
        self.echo.set(text);
        self.last_echo = Some(Instant::now());
        self.env.active_editor().show_cursor();
    }

    fn clear_echo(&mut self) {
        if let Some(_) = self.last_echo.take() {
            self.echo.clear();
            self.env.active_editor().show_cursor();
        }
    }

    fn resize_echo(&mut self) {
        if let Some(_) = self.last_echo {
            self.echo.resize();
            self.env.active_editor().show_cursor();
        }
    }

    fn set_question(&mut self, prompt: &str, answer_fn: Box<AnswerFn>) {
        self.input.enable(prompt);
        self.question = Some(answer_fn);
    }

    fn clear_question(&mut self) {
        if let Some(_) = self.question.take() {
            self.input.disable();
            self.env.active_editor().show_cursor();
        }
    }

    fn resize_question(&mut self) {
        if let Some(_) = self.question {
            self.input.resize();
        }
    }
}
