//! Basic types representing size and point information.

use std::fmt::{self, Display, Formatter};
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

impl Display for Size {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
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
