//! Main controller.
use crate::bind::Bindings;
use crate::echo::Echo;
use crate::editor::Align;
use crate::env::{Environment, Focus};
use crate::error::Result;
use crate::input::{Directive, InputEditor};
use crate::key::{Key, Keyboard, CTRL_G};
use crate::op::{self, Action, AnswerFn};
use crate::term;
use crate::workspace::{Placement, Workspace};
use crate::{PACKAGE_NAME, PACKAGE_VERSION};
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

enum Step {
    Continue,
    Quit,
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

    pub fn new(keyboard: Keyboard, bindings: Bindings, workspace: Workspace) -> Controller {
        let workspace = workspace.to_ref();
        let env = Environment::new(workspace.clone());
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

    /// Opens the collection of `files`, placing each successive editor at the bottom
    /// of the workspace.
    pub fn open(&mut self, files: &Vec<String>) -> Result<()> {
        let view_id = self.env.get_active();
        for (i, path) in files.iter().enumerate() {
            let editor = op::open_editor(path)?;
            if i == 0 {
                self.env.set_editor(editor, Align::Auto);
            } else {
                self.env.open_editor(editor, Placement::Bottom, Align::Auto);
            }
        }
        self.env.set_active(Focus::To(view_id));
        Ok(())
    }

    /// Runs the main processing loop.
    ///
    /// This loop orchestrates the entire editing experience, reading sequences of
    /// [keys](Key) and calling their corresponding editing functions until instructed to
    /// quit.
    pub fn run(&mut self) -> Result<()> {
        self.set_echo(&format!(
            "{PACKAGE_NAME} {PACKAGE_VERSION} | type C-h for help"
        ));
        self.show_cursor();
        loop {
            let key = self.keyboard.read()?;
            if key == Key::None {
                self.process_background()?;
            } else {
                if let Step::Quit = self.process_key(key)? {
                    break;
                } else {
                    self.show_cursor();
                }
            }
        }
        Ok(())
    }

    fn show_cursor(&mut self) {
        if let None = self.question {
            self.env.get_editor().borrow_mut().show_cursor();
        }
    }

    fn process_key(&mut self, key: Key) -> Result<Step> {
        if self.question.is_some() {
            self.process_question(key)
        } else {
            self.process_normal(key)
        }
    }

    fn process_normal(&mut self, key: Key) -> Result<Step> {
        if let Some(c) = self.possible_char(&key) {
            // Inserting text is statistically most prevalent scenario, so this
            // short circuits detection and bypasses normal indirection of key
            // binding.
            self.clear_echo();
            let mut editor = self.env.get_editor().borrow_mut();
            editor.clear_mark();
            editor.insert_char(c);
        } else if key == CTRL_G {
            self.clear_echo();
            if !self.clear_keys() {
                let mut editor = self.env.get_editor().borrow_mut();
                if let Some(_) = editor.clear_mark() {
                    editor.render();
                }
            }
        } else {
            self.key_seq.push(key.clone());
            if let Some(op_fn) = self.bindings.find(&self.key_seq) {
                match op_fn(&mut self.env)? {
                    Some(Action::Quit) => return Ok(Step::Quit),
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
        Ok(Step::Continue)
    }

    fn process_question(&mut self, key: Key) -> Result<Step> {
        let answer_fn = self.question.as_mut().unwrap();
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
            Some(Action::Quit) => return Ok(Step::Quit),
            Some(Action::Alert(text)) => {
                self.set_echo(text.as_str());
            }
            Some(Action::Question(prompt, answer_fn)) => {
                self.clear_echo();
                self.set_question(&prompt, answer_fn);
            }
            None => (),
        }
        Ok(Step::Continue)
    }

    fn process_background(&mut self) -> Result<Step> {
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
        Ok(Step::Continue)
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

    fn clear_keys(&mut self) -> bool {
        let cleared = self.key_seq.len() > 0;
        self.key_seq.clear();
        cleared
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
    }

    fn clear_echo(&mut self) {
        if let Some(_) = self.last_echo.take() {
            self.echo.clear();
        }
    }

    fn resize_echo(&mut self) {
        if let Some(_) = self.last_echo {
            self.echo.resize();
        }
    }

    fn set_question(&mut self, prompt: &str, answer_fn: Box<AnswerFn>) {
        self.input.enable(prompt);
        self.question = Some(answer_fn);
    }

    fn clear_question(&mut self) {
        if let Some(_) = self.question.take() {
            self.input.disable();
        }
    }

    fn resize_question(&mut self) {
        if let Some(_) = self.question {
            self.input.resize();
        }
    }
}
