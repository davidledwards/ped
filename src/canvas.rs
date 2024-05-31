//! Represents visible content of buffers.

use crate::color::Color;
use std::ops::Add;

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

impl Add for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Point {
        Point::new(self.row + rhs.row, self.col + rhs.col)
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

#[derive(Debug)]
pub struct Canvas {
    rows: u32,
    cols: u32,
    content: Vec<Cell>,
}

impl Canvas {
    pub fn new(rows: u32, cols: u32) -> Canvas {
        assert!(rows > 0);
        assert!(cols > 0);
        Canvas {
            rows,
            cols,
            content: vec![Cell::EMPTY; (rows * cols) as usize],
        }
    }

    pub fn row(&self, row: u32) -> &[Cell] {
        assert!(row < self.rows);
        let start = row * self.cols;
        let end = start + self.cols;
        &self.content[(start as usize)..(end as usize)]
    }

    pub fn row_mut(&mut self, row: u32) -> &mut [Cell] {
        assert!(row < self.rows);
        let start = row * self.cols;
        let end = start + self.cols;
        &mut self.content[(start as usize)..(end as usize)]
    }

    pub fn cell_mut(&mut self, row: u32, col: u32) -> &mut Cell {
        assert!(row < self.rows);
        assert!(col < self.cols);
        &mut self.content[(row * self.cols + col) as usize]
    }

    pub fn clear(&mut self) {
        self.content.fill(Cell::EMPTY);
    }

    // Apply differences in other with respect to this canvas and return a vector of
    // those differences.
    //
    // Note that this canvas will be equivalent to other upon return.
    pub fn reconcile(&mut self, other: &Canvas) -> Vec<(Point, Cell)> {
        assert!(self.rows == other.rows);
        assert!(self.cols == other.cols);

        let mut changes = Vec::new();
        for i in 0..self.content.len() {
            if self.content[i] != other.content[i] {
                changes.push((
                    Point::new((i as u32) / self.cols, (i as u32) % self.cols),
                    other.content[i],
                ));
                self.content[i] = other.content[i];
            }
        }
        changes
    }

    pub fn move_rows(&mut self, from_row: u32, to_row: u32, rows: u32) {
        debug_assert!(from_row < self.rows);
        debug_assert!(to_row < self.rows);
        debug_assert!(if from_row < to_row {
            to_row + rows <= self.rows
        } else {
            from_row + rows <= self.rows
        });

        let start = (from_row * self.cols) as usize;
        let end = start + (rows * self.cols) as usize;
        let dest = (to_row * self.cols) as usize;
        self.content.copy_within(start..end, dest);
    }

    pub fn clear_rows(&mut self, start_row: u32, end_row: u32) {
        debug_assert!(start_row < self.rows);
        debug_assert!(end_row <= self.rows);
        debug_assert!(start_row <= end_row);

        let start = (start_row * self.cols) as usize;
        let end = (end_row * self.cols) as usize;
        self.content[start..end].fill(Cell::EMPTY);
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter {
            canvas: &self,
            row: 0,
            col: 0,
            index: 0,
        }
    }

    pub fn row_iter(&self) -> RowIter<'_> {
        RowIter {
            canvas: &self,
            row: 0,
        }
    }
}

pub struct RowIter<'a> {
    canvas: &'a Canvas,
    row: u32,
}

pub struct ColIter<'a> {
    canvas: &'a Canvas,
    row_start: usize,
    col: u32,
}

impl<'a> Iterator for RowIter<'a> {
    type Item = (u32, ColIter<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.canvas.rows {
            let row = self.row;
            self.row += 1;
            Some((
                row,
                ColIter {
                    canvas: &self.canvas,
                    row_start: (row * self.canvas.cols) as usize,
                    col: 0,
                },
            ))
        } else {
            None
        }
    }
}

impl<'a> Iterator for ColIter<'a> {
    type Item = (u32, Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.col < self.canvas.cols {
            let col = self.col;
            self.col += 1;
            Some((col, self.canvas.content[self.row_start + (col as usize)]))
        } else {
            None
        }
    }
}

pub struct Iter<'a> {
    canvas: &'a Canvas,
    row: u32,
    col: u32,
    index: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Point, Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.canvas.rows {
            let item = Some((
                Point::new(self.row, self.col),
                self.canvas.content[self.index],
            ));
            self.col += 1;
            if self.col == self.canvas.cols {
                self.row += 1;
                self.col = 0;
            }
            self.index += 1;
            item
        } else {
            None
        }
    }
}
