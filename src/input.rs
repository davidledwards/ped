//! Line editor.
use crate::canvas::Canvas;
use crate::display::{Display, Point, Size};
use crate::grid::Cell;
use crate::key::{Ctrl, Key, Shift};
use crate::theme::ThemeRef;

use std::cmp;

pub struct LineEditor {
    /// The origin of the line editor.
    origin: Point,

    /// The number of columns reserved for the prompt area, which may be less than
    /// the length of [`prompt`] itself.
    prompt_cols: u32,

    /// The number of columns reserved for the input area, which is never less than
    /// [`MIN_COLS`](Self::MIN_COLS).
    buf_cols: u32,

    /// A reference to the color theme.
    theme: ThemeRef,

    /// The prompt provided by the caller.
    prompt: String,

    /// The input buffer.
    buf: Vec<char>,

    /// The current position in [`buf`] corresponding to [`cursor`].
    pos: usize,

    /// The position of the cursor on the visible canvas.
    cursor: u32,

    /// The canvas representing the input buffer.
    canvas: Canvas,

    /// A predefined blank cell honoring the color [`theme`].
    blank_cell: Cell,
}

pub enum Directive {
    Continue,
    Accept,
    Cancel,
}

impl LineEditor {
    /// A lower bound on the number of columns allocated to the line editor.
    const MIN_COLS: u32 = 2;

    pub fn new(origin: Point, cols: u32, theme: ThemeRef, prompt: &str) -> LineEditor {
        let (prompt_cols, buf_cols) = Self::calc_sizes(cols, prompt);
        let canvas = Canvas::new(origin + Size::cols(prompt_cols), Size::new(1, buf_cols));
        let blank_cell = Cell::new(' ', theme.text_color);
        let mut this = LineEditor {
            origin,
            prompt_cols,
            buf_cols,
            theme,
            prompt: prompt.to_string(),
            buf: Vec::new(),
            pos: 0,
            cursor: 0,
            canvas,
            blank_cell,
        };
        this.draw();
        this
    }

    pub fn draw(&mut self) {
        self.draw_prompt();
        self.draw_input();
    }

    /// Resizes the line editor using the revised `origin` and `cols`.
    pub fn resize(&mut self, origin: Point, cols: u32) {
        self.origin = origin;
        (self.prompt_cols, self.buf_cols) = Self::calc_sizes(cols, &self.prompt);
        self.canvas = Canvas::new(
            self.origin + Size::cols(self.prompt_cols),
            Size::new(1, self.buf_cols),
        );
        self.cursor = cmp::min(self.cursor, self.buf_cols);
        self.draw();
    }

    pub fn process_key(&mut self, key: &Key) -> Directive {
        match key {
            Key::Char(c) => {
                self.buf.insert(self.pos, *c);
                self.pos += 1;
                self.cursor = cmp::min(self.cursor + 1, self.buf_cols - 1);
                self.draw_input();
            }
            Key::Control(13) => {
                // RET
                return Directive::Accept;
            }
            Key::Control(8) | Key::Control(127) => {
                // ctrl-h | DEL
                // delete char to left
                if self.pos > 0 {
                    self.pos -= 1;
                    self.buf.remove(self.pos);
                    self.cursor = self.cursor.saturating_sub(1);
                    self.draw_input();
                }
            }
            Key::Control(4) => {
                // ctrl-d
                // delete char to right
                if self.pos < self.buf.len() {
                    self.buf.remove(self.pos);
                    self.draw_input();
                }
            }
            Key::Control(10) => {
                // ctrl-j
                // delete cursor to start of line
                if self.pos > 0 {
                    self.buf.drain(0..self.pos);
                    self.pos = 0;
                    self.cursor = 0;
                    self.draw_input();
                }
            }
            Key::Control(11) => {
                // ctrl-k
                // delete cursor to end of line
                if self.pos < self.buf.len() {
                    self.buf.truncate(self.pos);
                    self.draw_input();
                }
            }
            Key::Control(6) | Key::Right(Shift::Off, Ctrl::Off) => {
                // ctrl-f | -->
                // forward
                if self.pos < self.buf.len() {
                    self.pos += 1;
                    self.cursor = cmp::min(self.cursor + 1, self.buf_cols - 1);
                    self.draw_input();
                }
            }
            Key::Control(2) | Key::Left(Shift::Off, Ctrl::Off) => {
                // ctrl-b | <--
                // backward
                if self.pos > 0 {
                    self.pos -= 1;
                    self.cursor = self.cursor.saturating_sub(1);
                    self.draw_input();
                }
            }
            Key::Control(1) | Key::Home(Shift::Off, Ctrl::Off) => {
                // ctrl-a | HOME
                // start of line
                if self.pos > 0 {
                    self.pos = 0;
                    self.cursor = 0;
                    self.draw_input();
                }
            }
            Key::Control(5) | Key::End(Shift::Off, Ctrl::Off) => {
                // ctrl-e | END
                // end of line
                if self.pos < self.buf.len() {
                    self.pos = self.buf.len();
                    self.cursor = cmp::min(self.pos as u32, self.buf_cols - 1);
                    self.draw_input();
                }
            }
            Key::Control(7) => {
                // ctrl-g
                return Directive::Cancel;
            }
            _ => {
                // ignore everything else
            }
        }
        Directive::Continue
    }

    /// Takes as input the number of `cols` allocated to the entire line editor,
    /// as well as the `prompt`, and returns a tuple with the following calculations:
    /// - the number of columns allocated to the prompt
    /// - the number of columns allocated to the buffer
    ///
    /// Note that `cols` is possibly revised to ensure the value is never less than
    /// [`Self::MIN_COLS`].
    fn calc_sizes(cols: u32, prompt: &str) -> (u32, u32) {
        let cols = cmp::max(cols, Self::MIN_COLS);

        // Include trailing space after prompt in calculations.
        let prompt_len = prompt.chars().count() as u32 + 1;

        // Desired number of columns to show entire prompt while also ensuring minimum
        // size constraint for buffer area.
        let desired_cols = prompt_len + Self::MIN_COLS;
        let (prompt_cols, buf_cols) = if desired_cols > cols {
            // Clip prompt area so as not to exceed total width of input area.
            (prompt_len - (desired_cols - cols), Self::MIN_COLS)
        } else {
            // Give available space to buffer area.
            (prompt_len, cols - prompt_len)
        };
        if prompt_cols == 1 {
            // Since trailing space was included in size calculation, this condition
            // would only show trailing space, so just give it to buffer area.
            (0, buf_cols + 1)
        } else {
            (prompt_cols, buf_cols)
        }
    }

    fn draw_prompt(&mut self) {
        if self.prompt_cols > 0 {
            let prompt = self
                .prompt
                .chars()
                .take(self.prompt_cols as usize - 1)
                .collect::<String>();

            Display::new(self.origin)
                .set_cursor(Point::ORIGIN)
                .set_color(self.theme.prompt_color)
                .write_str(prompt.as_str())
                .write(' ')
                .send();
        }
    }

    fn draw_input(&mut self) {
        // Determine slice of buffer visible on canvas.
        let start = self.pos - self.cursor as usize;
        let end = cmp::min(start + self.buf_cols as usize, self.buf.len());

        // Write characters to canvas.
        for (col, c) in self.buf[start..end].iter().enumerate() {
            let cell = Cell::new(*c, self.theme.text_color);
            self.canvas.set_cell(0, col as u32, cell);
        }

        // Clear unused area on canvas.
        let cols = (end - start) as u32;
        if cols < self.buf_cols {
            self.canvas.fill_row_from(0, cols, self.blank_cell);
        }

        // Send pending changes to canvas and set new cursor position.
        self.canvas.draw();
        self.canvas.set_cursor(Point::new(0, self.cursor));
    }
}
