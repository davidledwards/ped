//! Implements the main controller for the entire editing experience.
//!
//! In general, the controller manages the workspace, reads key sequences from the
//! terminal, calls editing functions bound to those keys, and orchestrates the process
//! of soliciting input from the user. It also detects changes in the terminal size and
//! resizes the workspace accordingly.
//!
//! The controller is essentially a loop that runs until a _quit_ directive is given.

use crate::config::ConfigurationRef;
use crate::echo::Echo;
use crate::editor::{Align, ImmutableEditor};
use crate::env::{Environment, Focus};
use crate::error::Result;
use crate::etc::{PACKAGE_NAME, PACKAGE_VERSION};
use crate::input::{Directive, InputEditor};
use crate::key::{self, CTRL_G, Key, Keyboard, Shift};
use crate::op::{self, Action};
use crate::size::Point;
use crate::sys::{self, AsString};
use crate::term;
use crate::user::Question;
use crate::workspace::{Placement, Workspace};
use std::time::Instant;

/// The primary control point for coordinating user interaction and editing operations.
pub struct Controller {
    /// A reference to the editor configuration.
    config: ConfigurationRef,

    /// A keyboard for reading [keys](Key).
    keyboard: Keyboard,

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
    question: Option<Box<dyn Question>>,

    /// An optional time capturing the last terminal size change event.
    term_changed: Option<Instant>,
}

enum Step {
    Continue,
    Quit,
}

type SyntheticFn<'a> = Box<dyn FnMut(&mut Environment) + 'a>;

impl Controller {
    /// Number of milliseconds controller waits before resizing workspace after it notices a
    /// change.
    const TERM_CHANGE_DELAY: u128 = 100;

    pub fn new(keyboard: Keyboard, workspace: Workspace) -> Controller {
        let config = workspace.config.clone();
        let workspace = workspace.into_ref();
        let env = Environment::new(workspace.clone());
        let echo = Echo::new(workspace.clone());
        let input = InputEditor::new(workspace.clone());

        Controller {
            config,
            keyboard,
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
    pub fn open(&mut self, files: &[String]) -> Result<()> {
        let view_id = self.env.get_active_view_id();
        for (i, path) in files.iter().enumerate() {
            let path = sys::canonicalize(sys::working_dir().join(path)).as_string();
            let editor = op::open_editor(self.config.clone(), &path)?;
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
    pub fn run(&mut self) {
        self.set_echo(self.welcome());
        self.show_cursor();
        loop {
            let key = self.keyboard.read().unwrap_or(Key::None);
            if key == Key::None {
                self.process_background();
            } else if let Step::Quit = self.process_key(key) {
                break;
            } else {
                self.show_cursor();
            }
        }
    }

    fn welcome(&self) -> String {
        if let Some(help_key) = self.config.bindings.find_key("help").first() {
            format!(
                "{PACKAGE_NAME} {PACKAGE_VERSION} | type {} for help, C-q to quit",
                key::pretty(help_key)
            )
        } else {
            format!("{PACKAGE_NAME} {PACKAGE_VERSION} | type C-q to quit")
        }
    }

    fn show_cursor(&mut self) {
        if self.question.is_none() {
            self.env.get_active_editor().borrow_mut().show_cursor();
        }
    }

    fn process_key(&mut self, key: Key) -> Step {
        if self.question.is_some() {
            self.process_question(key)
        } else {
            self.process_normal(key)
        }
    }

    fn process_normal(&mut self, key: Key) -> Step {
        if let Some(c) = self.possible_char(&key) {
            // Inserting text is statistically most prevalent scenario, so this short
            // circuits detection and bypasses normal indirection of key binding.
            match op::insert_char(&mut self.env, c) {
                Some(Action::Echo(text)) => self.set_echo(text),
                _ => self.clear_echo(),
            }
        } else if key == CTRL_G {
            self.clear_echo();
            if !self.clear_keys() {
                let mut editor = self.env.get_active_editor().borrow_mut();
                if editor.clear_mark().is_some() {
                    editor.render();
                }
            }
        } else if let Some(mut scroll_fn) = Self::possible_scroll(&key) {
            self.clear_echo();
            scroll_fn(&mut self.env);
        } else if let Some(mut button_fn) = Self::possible_button(&key) {
            self.clear_echo();
            button_fn(&mut self.env);
        } else {
            self.key_seq.push(key.clone());
            if let Some(op_fn) = self.config.bindings.find(&self.key_seq) {
                match op_fn(&mut self.env) {
                    Some(Action::Quit) => return Step::Quit,
                    Some(Action::Redraw) => self.redraw(),
                    Some(Action::Echo(text)) => self.set_echo(text),
                    Some(Action::Question(question)) => self.set_question(question),
                    None => self.clear_echo(),
                }
                self.clear_keys();
            } else if self.config.bindings.is_prefix(&self.key_seq) {
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
        Step::Continue
    }

    fn process_question(&mut self, key: Key) -> Step {
        let question = self.question.as_mut().unwrap();
        let action = if key == CTRL_G {
            let action = question.respond(&mut self.env, None);
            self.clear_question();
            action
        } else if let Some(mut scroll_fn) = Self::possible_scroll(&key) {
            scroll_fn(&mut self.env);
            self.input.show_cursor();
            None
        } else {
            match self.input.process_key(&key) {
                Directive::Continue => {
                    let value = self.input.value();
                    if let Some(hint) = question.react(&mut self.env, &value, &key) {
                        self.input.set_hint(hint);
                    }
                    self.input.show_cursor();
                    None
                }
                Directive::Ignore => None,
                Directive::Accept => {
                    let value = self.input.value();
                    let action = question.respond(&mut self.env, Some(&value));
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
            Some(Action::Quit) => return Step::Quit,
            Some(Action::Redraw) => self.redraw(),
            Some(Action::Echo(text)) => self.set_echo(text),
            Some(Action::Question(question)) => self.set_question(question),
            None => (),
        }
        Step::Continue
    }

    fn process_background(&mut self) -> Step {
        self.term_changed = if term::size_changed() {
            // Restart clock when terminal size change detected.
            Some(Instant::now())
        } else if let Some(time) = self.term_changed.take() {
            // Defer resizing workspace for short period of time since human movement,
            // in practice, could generate rapid series of change events.
            if time.elapsed().as_millis() > Self::TERM_CHANGE_DELAY {
                self.resize();
                None
            } else {
                Some(time)
            }
        } else {
            // Tokenize editor contents when nothing else to do.
            let mut editor = self.env.get_active_editor().borrow_mut();
            if editor.tokenize() {
                editor.render();
                editor.show_cursor();
            }
            None
        };
        Step::Continue
    }

    /// An efficient means of detecting the very common case of a single character,
    /// allowing the controller to optimize its handling.
    fn possible_char(&self, key: &Key) -> Option<char> {
        if self.key_seq.is_empty()
            && let Key::Char(c) = key
        {
            Some(*c)
        } else {
            None
        }
    }

    /// Returns an optional function to call if `key` represents a scrolling event.
    fn possible_scroll(key: &Key) -> Option<SyntheticFn<'_>> {
        match key {
            Key::ScrollUp(shift, row, col) => Some(Box::new(|env: &mut Environment| {
                op::track_up(env, Point::new(*row, *col), *shift == Shift::On);
            })),
            Key::ScrollDown(shift, row, col) => Some(Box::new(|env: &mut Environment| {
                op::track_down(env, Point::new(*row, *col), *shift == Shift::On);
            })),
            Key::ScrollLeft(shift, row, col) => Some(Box::new(|env: &mut Environment| {
                op::track_backward(env, Point::new(*row, *col), *shift == Shift::On);
            })),
            Key::ScrollRight(shift, row, col) => Some(Box::new(|env: &mut Environment| {
                op::track_forward(env, Point::new(*row, *col), *shift == Shift::On);
            })),
            _ => None,
        }
    }

    /// Returns an optional function to call if `key` represents a button event.
    fn possible_button(key: &Key) -> Option<SyntheticFn<'_>> {
        match key {
            Key::ButtonPress(row, col) => Some(Box::new(|env: &mut Environment| {
                op::set_focus(env, Point::new(*row, *col));
            })),
            Key::ButtonRelease(_, _) => Some(Box::new(|_: &mut Environment| {
                // Absorb since this event serves no purpose at this time.
            })),
            _ => None,
        }
    }

    fn redraw(&mut self) {
        self.env.redraw();
        self.clear_echo();
        self.show_cursor();
    }

    fn resize(&mut self) {
        self.env.resize();
        if self.last_echo.is_some() {
            self.echo.resize();
        } else if self.question.is_some() {
            self.input.resize();
        }
        self.show_cursor();
    }

    fn clear_keys(&mut self) -> bool {
        let cleared = self.key_seq.len() > 0;
        self.key_seq.clear();
        cleared
    }

    fn show_keys(&mut self) {
        self.set_echo(key::pretty(&self.key_seq));
    }

    fn show_undefined_keys(&mut self) {
        self.set_echo(format!("{}: undefined key", key::pretty(&self.key_seq)));
    }

    fn set_echo(&mut self, text: String) {
        self.echo.set(text);
        self.last_echo = Some(Instant::now());
    }

    fn clear_echo(&mut self) {
        if self.last_echo.take().is_some() {
            self.echo.clear();
        }
    }

    fn set_question(&mut self, question: Box<dyn Question>) {
        self.clear_echo();
        self.input.enable(&*question);
        self.question = Some(question);
    }

    fn clear_question(&mut self) {
        if self.question.take().is_some() {
            self.input.disable();
        }
    }
}
