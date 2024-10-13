//! Sends display instructions to terminal.
use crate::ansi;
use crate::color::Color;

use std::fmt;
use std::io::{self, Write};
use std::ops::{Add, Sub};

/// Represents the size of a 2-dimensional space expressed as `rows` and `cols`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Size {
    pub rows: u32,
    pub cols: u32,
}

impl Size {
    pub const ZERO: Size = Size::new(0, 0);

    pub const fn new(rows: u32, cols: u32) -> Size {
        Size { rows, cols }
    }

    pub const fn rows(rows: u32) -> Size {
        Size { rows, cols: 0 }
    }

    pub const fn cols(cols: u32) -> Size {
        Size { rows: 0, cols }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.rows, self.cols)
    }
}

impl From<(u32, u32)> for Size {
    fn from(value: (u32, u32)) -> Size {
        Size::new(value.0, value.1)
    }
}

impl Sub<Size> for Size {
    type Output = Size;

    fn sub(self, rhs: Size) -> Size {
        Size::new(self.rows - rhs.rows, self.cols - rhs.cols)
    }
}

/// Represent a point in a 2-dimensional space expressed as `row` and `col`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Point {
    pub row: u32,
    pub col: u32,
}

impl Point {
    pub const ORIGIN: Point = Point::new(0, 0);

    pub const fn new(row: u32, col: u32) -> Point {
        Point { row, col }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.row, self.col)
    }
}

impl Add<Point> for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Point {
        Point::new(self.row + rhs.row, self.col + rhs.col)
    }
}

impl Add<Size> for Point {
    type Output = Point;

    fn add(self, rhs: Size) -> Point {
        Point::new(self.row + rhs.rows, self.col + rhs.cols)
    }
}

impl Add<(u32, u32)> for Point {
    type Output = Point;

    fn add(self, rhs: (u32, u32)) -> Point {
        self + Point::new(rhs.0, rhs.1)
    }
}

/// A buffered abstraction over standard output that supports sending content to the
/// display in a structured way.
///
/// Cursor operations are relative to an [origin](`Point`) that is provided during
/// instantiation of the display.
pub struct Display {
    origin: Point,
    out: String,
}

impl Display {
    /// Creates a display with `origin` as its reference point for cursor operations.
    pub fn new(origin: Point) -> Display {
        Display {
            origin,
            out: String::new(),
        }
    }

    /// Sends buffered changes to standard output.
    pub fn send(&mut self) {
        if self.out.len() > 0 {
            print!("{}", self.out);
            let _ = io::stdout().flush();
            self.out.clear();
        }
    }

    pub fn set_cursor(&mut self, cursor: Point) -> &mut Display {
        self.out
            .push_str(ansi::set_cursor(self.origin + cursor).as_str());
        self
    }

    pub fn set_color(&mut self, color: Color) -> &mut Display {
        self.out.push_str(ansi::set_color(color).as_str());
        self
    }

    pub fn write(&mut self, c: char) -> &mut Display {
        self.out.push(c);
        self
    }

    pub fn write_str(&mut self, text: &str) -> &mut Display {
        self.out.push_str(text);
        self
    }
}
