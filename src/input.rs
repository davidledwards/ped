//! Input reader.
use crate::canvas::Canvas;
use crate::color::Color;
use crate::display::{Display, Point, Size};
use crate::grid::Cell;
use crate::key::{Ctrl, Key, Shift};

use std::cmp;

pub struct Input {
    origin: Point,
    cols: u32,
    prompt: String,
    buf: Vec<char>,
    pos: usize,
    cursor: u32,
    prompt_cols: u32,
    buf_cols: u32,
    canvas: Canvas,
}

pub enum Status {
    Continue,
    Accept,
    Quit,
}

impl Input {
    const MIN_COLS: u32 = 2;

    /// Takes as input the number of `cols` allocated to the entire input area,
    /// as well as the `prompt`, and returns a tuple with the following calculations:
    /// - a possibly revised value for `cols` to ensure the value is never less than
    ///   [`Self::MIN_COLS`]
    /// - the number of columns allocated to the prompt
    /// - the number of columns allocated to the buffer
    fn calc_sizes(cols: u32, prompt: &str) -> (u32, u32, u32) {
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
            (cols, 0, buf_cols + 1)
        } else {
            (cols, prompt_cols, buf_cols)
        }
    }

    pub fn new(origin: Point, cols: u32, prompt: &str) -> Input {
        let (cols, prompt_cols, buf_cols) = Self::calc_sizes(cols, prompt);

        let canvas = Canvas::new(origin + Size::cols(prompt_cols), Size::new(1, buf_cols));

        let mut this = Input {
            origin,
            cols,
            prompt: prompt.to_string(),
            buf: Vec::new(),
            pos: 0,
            cursor: 0,
            prompt_cols,
            buf_cols,
            canvas,
        };
        this.draw_prompt();
        this.draw_input();
        this
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
                .set_color(Color::new(2, 232))
                .write_str(prompt.as_str())
                .write(' ')
                .send();
        }
    }

    fn draw_input(&mut self) {
        let start_pos = self.pos - self.cursor as usize;
        let end_pos = cmp::min(start_pos + self.canvas.size().cols as usize, self.buf.len());
        for (col, c) in self.buf[start_pos..end_pos].iter().enumerate() {
            self.canvas
                .set_cell(0, col as u32, Cell::new(*c, Color::new(15, 233)));
        }
        let n = end_pos - start_pos;
        if n < self.canvas.size().cols as usize {
            self.canvas
                .fill_row_from(0, n as u32, Cell::new(' ', Color::new(15, 233)));
        }
        self.canvas.draw();
        self.canvas.set_cursor(Point::new(0, self.cursor));
    }

    pub fn process_key(&mut self, key: &Key) -> Status {
        match key {
            Key::Char(c) => {
                self.buf.insert(self.pos, *c);
                self.pos += 1;
                self.cursor = cmp::min(self.cursor + 1, self.canvas.size().cols - 1);
                self.draw_input();
            }
            Key::Control(13) => {
                // RET
                return Status::Accept;
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
                    self.cursor = cmp::min(self.cursor + 1, self.canvas.size().cols - 1);
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
                    self.cursor = cmp::min(self.pos as u32, self.canvas.size().cols - 1);
                    self.draw_input();
                }
            }
            _ => {
                // ignore everything else
            }
        }
        Status::Continue
    }
}
