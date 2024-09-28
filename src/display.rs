//! Sends display instructions to terminal.
use crate::ansi;
use crate::color::Color;

use std::io::{self, Write};
use std::ops::{Add, Sub};

#[derive(Copy, Clone, Debug)]
pub struct Size {
    pub rows: u32,
    pub cols: u32,
}

impl Size {
    pub const fn new(rows: u32, cols: u32) -> Size {
        Size { rows, cols }
    }

    pub fn rows(rows: u32) -> Size {
        Size { rows, cols: 0 }
    }
}

impl Sub<Size> for Size {
    type Output = Size;

    fn sub(self, rhs: Size) -> Size {
        Size::new(self.rows - rhs.rows, self.cols - rhs.cols)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Point {
    pub row: u32,
    pub col: u32,
}

impl Point {
    pub const ORIGIN: Point = Point { row: 0, col: 0 };

    pub fn new(row: u32, col: u32) -> Point {
        Point { row, col }
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

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Cell {
    pub value: char,
    pub color: Color,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        value: '\0',
        color: Color::ZERO,
    };

    pub fn new(value: char, color: Color) -> Cell {
        Cell { value, color }
    }
}

impl Default for Cell {
    fn default() -> Cell {
        Cell::EMPTY
    }
}

pub struct Display {
    origin: Point,
    out: String,
}

impl Display {
    pub fn new(origin: Point) -> Display {
        Display {
            origin,
            out: String::new(),
        }
    }

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

    pub fn write_str(&mut self, text: &str) -> &mut Display {
        self.out.push_str(text);
        self
    }

    pub fn write_cell(
        &mut self,
        p: Point,
        cell: Cell,
        hint: Option<(Point, Cell)>,
    ) -> &mut Display {
        match hint {
            Some((prev_p, prev_cell)) => {
                if p.row != prev_p.row || p.col != prev_p.col + 1 {
                    self.set_cursor(p);
                }
                if cell.color != prev_cell.color {
                    self.out.push_str(ansi::set_color(cell.color).as_str());
                }
            }
            None => {
                self.set_cursor(p);
                self.out.push_str(ansi::set_color(cell.color).as_str());
            }
        }
        self.out.push(cell.value);
        self
    }
}
