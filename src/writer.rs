//! Writes display instructions to the terminal.
use crate::ansi;
use crate::color::Color;
use crate::size::Point;

use std::io::{self, Write};

/// A buffered abstraction over standard output that sends content to the terminal
/// in a structured way.
///
/// Cursor operations are relative to an [origin](`Point`) that is provided during
/// instantiation of the writer.
pub struct Writer {
    origin: Point,
    out: String,
}

impl Writer {
    /// Creates a writer with `origin` as its reference point for cursor operations.
    pub fn new(origin: Point) -> Writer {
        Writer {
            origin,
            out: String::new(),
        }
    }

    /// Creates a writer with `origin` as its reference point for cursor operations,
    /// and additionally calls [`set_origin`](Self::set_origin).
    pub fn new_at(origin: Point) -> Writer {
        let mut this = Self::new(origin);
        this.set_origin();
        this
    }

    /// Sends buffered changes to standard output.
    pub fn send(&mut self) {
        if self.out.len() > 0 {
            print!("{}", self.out);
            let _ = io::stdout().flush();
            self.out.clear();
        }
    }

    pub fn set_origin(&mut self) -> &mut Writer {
        self.set_cursor(Point::ORIGIN)
    }

    pub fn set_cursor(&mut self, cursor: Point) -> &mut Writer {
        self.out
            .push_str(ansi::set_cursor(self.origin + cursor).as_str());
        self
    }

    pub fn set_color(&mut self, color: Color) -> &mut Writer {
        self.out.push_str(ansi::set_color(color).as_str());
        self
    }

    pub fn write(&mut self, c: char) -> &mut Writer {
        self.out.push(c);
        self
    }

    pub fn write_str(&mut self, text: &str) -> &mut Writer {
        self.out.push_str(text);
        self
    }
}