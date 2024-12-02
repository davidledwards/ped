//! An editor for soliciting input.
//!
//! An input editor uses the shared region of the workspace as its canvas, and can be
//! described as a simplified editing experience aimed at capturing input from the
//! user.
//!
//! A key design element is the integration of a [`Completer`], which allows the
//! creation of arbitrarily sophisticated input mechanisms, such as file completion.

use crate::canvas::Canvas;
use crate::grid::Cell;
use crate::key::*;
use crate::size::{Point, Size};
use crate::user::{self, Completer};
use crate::workspace::WorkspaceRef;
use crate::writer::Writer;
use std::cmp;

pub struct InputEditor {
    /// Associated workspace.
    workspace: WorkspaceRef,

    /// Contains a prompt when the input editor is enabled, otherwise `None`.
    prompt: Option<String>,

    /// A completer assigned when the editor is enabled, otherwise it assumes the value
    /// of [`null_completer`](user::null_completer).
    completer: Box<dyn Completer>,

    /// Represents the number of columns reserved for the prompt area when enabled,
    /// otherwise `0`.
    prompt_cols: u32,

    /// Represents the number of columns reserved for the input area when enabled,
    /// otherwise `0`.
    input_cols: u32,

    /// The canvas representing the input buffer when enabled, otherwise
    /// [`zero`](Canvas::zero).
    canvas: Canvas,

    /// The input buffer.
    input: Vec<char>,

    /// The length of the user-provided portion of `input`, which is always less than
    /// or equal to `input.len()`.
    len: usize,

    /// The current position in `input` corresponding to [`cursor`](Self::cursor).
    pos: usize,

    /// The position of the cursor on the visible canvas.
    cursor: u32,

    /// An optional *hint* that is appended to the user-provided portion of `input`.
    hint: Option<String>,
}

pub enum Directive {
    Continue,
    Accept,
    Cancel,
}

impl InputEditor {
    /// A lower bound on the number of columns allocated to the input editor.
    const MIN_COLS: u32 = 2;

    pub fn new(workspace: WorkspaceRef) -> InputEditor {
        InputEditor {
            workspace,
            prompt: None,
            completer: user::null_completer(),
            prompt_cols: 0,
            input_cols: 0,
            canvas: Canvas::zero(),
            input: Vec::new(),
            len: 0,
            pos: 0,
            cursor: 0,
            hint: None,
        }
    }

    /// Enables the editor by associating a `prompt` and a `completer`.
    pub fn enable(&mut self, prompt: &str, completer: Box<dyn Completer>) {
        self.prompt = Some(prompt.to_string());
        self.completer = completer;
        self.set_sizes();
        self.input.clear();
        self.len = 0;
        self.pos = 0;
        self.cursor = 0;
        self.hint = self.completer.prepare();
        if let Some(hint) = self.hint.as_ref() {
            self.input.extend(hint.chars());
        }
        self.draw();
    }

    /// Disables the editor and clears the area on the workspace.
    pub fn disable(&mut self) {
        self.prompt = None;
        self.completer = user::null_completer();
        self.set_sizes();
        self.input.clear();
        self.len = 0;
        self.pos = 0;
        self.cursor = 0;
        self.hint = None;
        self.draw();
    }

    /// Returns the contents of the user-provided portion of the input buffer.
    pub fn value(&self) -> String {
        self.input.iter().take(self.len).collect()
    }

    pub fn draw(&mut self) {
        if let Some(_) = self.prompt {
            self.draw_prompt();
            self.draw_input();
        } else {
            self.workspace.borrow_mut().clear_shared();
        }
    }

    /// Resizes the input editor by reprobing the associated workspace.
    pub fn resize(&mut self) {
        self.set_sizes();
        self.cursor = cmp::min(self.cursor, self.input_cols.saturating_sub(1));
        self.draw();
    }

    pub fn process_key(&mut self, key: &Key) -> Directive {
        match *key {
            Key::Char(c) => {
                self.input.insert(self.pos, c);
                self.len += 1;
                self.pos += 1;
                self.cursor = cmp::min(self.cursor + 1, self.input_cols - 1);
                self.evaluate();
                self.draw_input();
            }
            DELETE => {
                // Delete character before cursor.
                if self.pos > 0 {
                    self.len -= 1;
                    self.pos -= 1;
                    self.input.remove(self.pos);
                    self.cursor = self.cursor.saturating_sub(1);
                    self.evaluate();
                    self.draw_input();
                }
            }
            CTRL_D => {
                // Delete character after cursor.
                if self.pos < self.len {
                    self.input.remove(self.pos);
                    self.len -= 1;
                    self.evaluate();
                    self.draw_input();
                }
            }
            CTRL_J => {
                // Delete characters from cursor to start of line.
                if self.pos > 0 {
                    self.input.drain(0..self.pos);
                    self.len -= self.pos;
                    self.pos = 0;
                    self.cursor = 0;
                    self.evaluate();
                    self.draw_input();
                }
            }
            CTRL_K => {
                // Delete characters from cursor to end of line.
                if self.pos < self.len {
                    self.input.drain(self.pos..self.len);
                    self.len = self.pos;
                    self.evaluate();
                    self.draw_input();
                }
            }
            CTRL_F | RIGHT => {
                // Move cursor forward.
                if self.pos < self.len {
                    self.pos += 1;
                    self.cursor = cmp::min(self.cursor + 1, self.input_cols - 1);
                    self.draw_input();
                }
            }
            CTRL_B | LEFT => {
                // Move cursor backward.
                if self.pos > 0 {
                    self.pos -= 1;
                    self.cursor = self.cursor.saturating_sub(1);
                    self.draw_input();
                }
            }
            CTRL_A | HOME => {
                // Move cursor to start of line.
                if self.pos > 0 {
                    self.pos = 0;
                    self.cursor = 0;
                    self.draw_input();
                }
            }
            CTRL_E | END => {
                // Move cursor to end of line.
                if self.pos < self.len {
                    self.pos = self.len;
                    self.cursor = cmp::min(self.pos as u32, self.input_cols - 1);
                    self.draw_input();
                }
            }
            TAB => {
                self.suggest();
                self.draw_input();
            }
            CTRL_M => {
                if self.accept() {
                    return Directive::Accept;
                }
            }
            CTRL_G => {
                return Directive::Cancel;
            }
            _ => (),
        }
        Directive::Continue
    }

    /// Calls the attached completer to evaluate the input value in its current form.
    fn evaluate(&mut self) {
        if let Some(hint) = (self.completer).evaluate(&self.value()) {
            self.input.truncate(self.len);
            self.input.extend(hint.chars());
            self.hint = Some(hint);
        } else if let Some(_) = self.hint.take() {
            self.input.truncate(self.len);
        }
    }

    /// Calls the attached completer to make a suggestion based on the input value in
    /// its current form.
    fn suggest(&mut self) {
        match (self.completer).suggest(&self.value()) {
            (Some(replace), hint) => {
                self.input = replace.chars().collect();
                self.len = self.input.len();
                if let Some(hint) = hint.as_ref() {
                    self.input.extend(hint.chars());
                }
                self.pos = self.len;
                self.cursor = cmp::min(self.pos as u32, self.input_cols - 1);
                self.hint = hint;
            }
            (None, Some(hint)) => {
                self.input.truncate(self.len);
                self.input.extend(hint.chars());
                self.hint = Some(hint);
            }
            (None, None) => {
                if let Some(_) = self.hint.take() {
                    self.input.truncate(self.len);
                }
            }
        }
    }

    /// Calls the attached completer to accept or reject the input value in its
    /// form, returning `true` if the accepted and `false` otherwise.
    fn accept(&mut self) -> bool {
        if let Some(value) = (self.completer).accept(&self.value()) {
            self.input = value.chars().collect();
            self.len = self.input.len();
            self.hint = None;
            true
        } else {
            false
        }
    }

    /// Sets column sizes for the *prompt* and *input* areas, and allocates an
    /// appropriately-sized canvas.
    fn set_sizes(&mut self) {
        if let Some(ref prompt) = self.prompt {
            // Editor is enabled.
            let (origin, size) = self.workspace.borrow().shared_region();
            (self.prompt_cols, self.input_cols) = Self::calc_sizes(size.cols, prompt);
            self.canvas = Canvas::new(
                origin + Size::cols(self.prompt_cols),
                Size::new(1, self.input_cols),
            );
        } else {
            // Editor is disabled, so set everything to zero.
            self.prompt_cols = 0;
            self.input_cols = 0;
            self.canvas = Canvas::zero();
        }
    }

    /// Given the number of `cols` allocated to the entire input editor, as well as
    /// the `prompt`, and returns a tuple with the following calculations:
    /// - the number of columns allocated to the prompt area
    /// - the number of columns allocated to the input area
    ///
    /// Note that `cols` is possibly revised to ensure the value is never less than
    /// [`MIN_COLS`](Self::MIN_COLS).
    fn calc_sizes(cols: u32, prompt: &str) -> (u32, u32) {
        let cols = cmp::max(cols, Self::MIN_COLS);

        // Include trailing space after prompt in calculations.
        let prompt_len = prompt.chars().count() as u32 + 1;

        // Desired number of columns to show entire prompt while also ensuring minimum
        // size constraint for input area.
        let desired_cols = prompt_len + Self::MIN_COLS;
        let (prompt_cols, input_cols) = if desired_cols > cols {
            // Clip prompt area so as not to exceed total width of input area.
            (prompt_len - (desired_cols - cols), Self::MIN_COLS)
        } else {
            // Give available space to input area.
            (prompt_len, cols - prompt_len)
        };
        if prompt_cols == 1 {
            // Since trailing space was included in size calculation, this condition
            // would only show trailing space, so give it to input area.
            (0, input_cols + 1)
        } else {
            (prompt_cols, input_cols)
        }
    }

    fn draw_prompt(&mut self) {
        if self.prompt_cols > 0 {
            let prompt = self
                .prompt
                .as_ref()
                .unwrap()
                .chars()
                .take(self.prompt_cols as usize - 1)
                .collect::<String>();

            let (origin, _) = self.workspace.borrow().shared_region();
            let color = self.workspace.borrow().config().colors.prompt;
            Writer::new_at(origin)
                .set_color(color)
                .write_str(prompt.as_str())
                .write(' ')
                .send();
        }
    }

    fn draw_input(&mut self) {
        // Determine slice of input buffer visible on canvas.
        let start = self.pos - self.cursor as usize;
        let end = cmp::min(start + self.input_cols as usize, self.input.len());

        // Write user-provided section of text to canvas followed by optional hint,
        // since colors are distinct.
        let color = self.workspace.borrow().config().colors.text;
        let user_end = cmp::min(end, self.len);
        for (col, c) in self.input[start..user_end].iter().enumerate() {
            let cell = Cell::new(*c, color);
            self.canvas.set_cell(0, col as u32, cell);
        }
        let color = self.workspace.borrow().config().colors.echo;
        let hint_ofs = user_end - start;
        for (col, c) in self.input[user_end..end].iter().enumerate() {
            let cell = Cell::new(*c, color);
            self.canvas.set_cell(0, (hint_ofs + col) as u32, cell);
        }

        // Clear unused area on canvas.
        let cols = (end - start) as u32;
        if cols < self.input_cols {
            let cell = Cell::new(' ', self.workspace.borrow().config().colors.text);
            self.canvas.fill_row_from(0, cols, cell);
        }

        // Send pending changes to canvas and set new cursor position.
        self.canvas.draw();
        self.canvas.set_cursor(Point::new(0, self.cursor));
    }
}
