//! Basic types representing size and point information.

use std::fmt::{self, Display, Formatter};
use std::ops::{Add, Sub};

/// Represents the size of a 2-dimensional space expressed as _rows_ and _columns_.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Size {
    pub rows: u32,
    pub cols: u32,
}

impl Size {
    /// A size of (`0`, `0`).
    pub const ZERO: Size = Size::new(0, 0);

    /// Creates a size of (`rows`, `cols`).
    pub const fn new(rows: u32, cols: u32) -> Size {
        Size { rows, cols }
    }

    /// Creates a size of (`rows`, `0`).
    pub const fn rows(rows: u32) -> Size {
        Size { rows, cols: 0 }
    }

    /// Creates a size of (`0`, `cols`).
    pub const fn cols(cols: u32) -> Size {
        Size { rows: 0, cols }
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.rows, self.cols)
    }
}

impl Sub<Size> for Size {
    type Output = Size;

    fn sub(self, rhs: Size) -> Size {
        Size::new(self.rows - rhs.rows, self.cols - rhs.cols)
    }
}

/// Represent a point in a 2-dimensional space expressed as _row_ and _column_.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Point {
    pub row: u32,
    pub col: u32,
}

impl Point {
    /// A point of (`0`, `0`).
    pub const ORIGIN: Point = Point::new(0, 0);

    /// Creates a point of (`row`, `col`).
    pub const fn new(row: u32, col: u32) -> Point {
        Point { row, col }
    }

    /// Returns `true` if this point is adjacent to and follows `p`.
    #[inline(always)]
    pub fn follows(&self, p: Point) -> bool {
        self.row == p.row && self.col == p.col + 1
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.row, self.col)
    }
}

impl Add<Point> for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Point {
        Point::new(self.row + rhs.row, self.col + rhs.col)
    }
}

impl Sub<Point> for Point {
    type Output = Point;

    fn sub(self, rhs: Point) -> Point {
        Point::new(self.row - rhs.row, self.col - rhs.col)
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
