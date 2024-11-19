//! Input editor.
use crate::canvas::Canvas;
use crate::grid::Cell;
use crate::key::*;
use crate::size::{Point, Size};
use crate::workspace::WorkspaceRef;
use crate::writer::Writer;
use std::cmp;

pub struct InputEditor {
    /// Associated workspace.
    workspace: WorkspaceRef,

    /// Contains a prompt when the input editor is enabled, otherwise `None`.
    prompt: Option<String>,

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

    /// The current position in [`input`](Self::input) corresponding to
    /// [`cursor`](Self::cursor).
    pos: usize,

    /// The position of the cursor on the visible canvas.
    cursor: u32,
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
            prompt_cols: 0,
            input_cols: 0,
            canvas: Canvas::zero(),
            input: Vec::new(),
            pos: 0,
            cursor: 0,
        }
    }

    /// Enables the editor by associating `prompt`.
    pub fn enable(&mut self, prompt: &str) {
        self.prompt = Some(prompt.to_string());
        self.set_sizes();
        self.input.clear();
        self.pos = 0;
        self.cursor = 0;
        self.draw();
    }

    /// Disables the editor and clears the area on the workspace.
    pub fn disable(&mut self) {
        self.prompt = None;
        self.set_sizes();
        self.input.clear();
        self.pos = 0;
        self.cursor = 0;
        self.draw();
    }

    /// Returns the contents of the input buffer.
    pub fn buffer(&self) -> String {
        self.input.iter().collect()
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
                self.pos += 1;
                self.cursor = cmp::min(self.cursor + 1, self.input_cols - 1);
                self.draw_input();
            }
            CTRL_M => {
                return Directive::Accept;
            }
            CTRL_H | DELETE => {
                // Delete character left of cursor.
                if self.pos > 0 {
                    self.pos -= 1;
                    self.input.remove(self.pos);
                    self.cursor = self.cursor.saturating_sub(1);
                    self.draw_input();
                }
            }
            CTRL_D => {
                // Delete character right of cursor.
                if self.pos < self.input.len() {
                    self.input.remove(self.pos);
                    self.draw_input();
                }
            }
            CTRL_J => {
                // Delete characters left of cursor to start of line.
                if self.pos > 0 {
                    self.input.drain(0..self.pos);
                    self.pos = 0;
                    self.cursor = 0;
                    self.draw_input();
                }
            }
            CTRL_K => {
                // Delete characters right of cursor to end of line.
                if self.pos < self.input.len() {
                    self.input.truncate(self.pos);
                    self.draw_input();
                }
            }
            CTRL_F | RIGHT => {
                // Move cursor right.
                if self.pos < self.input.len() {
                    self.pos += 1;
                    self.cursor = cmp::min(self.cursor + 1, self.input_cols - 1);
                    self.draw_input();
                }
            }
            CTRL_B | LEFT => {
                // Move cursor left.
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
                if self.pos < self.input.len() {
                    self.pos = self.input.len();
                    self.cursor = cmp::min(self.pos as u32, self.input_cols - 1);
                    self.draw_input();
                }
            }
            CTRL_G => {
                return Directive::Cancel;
            }
            _ => (),
        }
        Directive::Continue
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

        // Write characters to canvas.
        let color = self.workspace.borrow().config().colors.text;
        for (col, c) in self.input[start..end].iter().enumerate() {
            let cell = Cell::new(*c, color);
            self.canvas.set_cell(0, col as u32, cell);
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
