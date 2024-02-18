//! Represents visible content of buffers.

use crate::color::Color;

#[derive(Copy, Clone, Debug)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

impl Point {
    pub fn new(row: usize, col: usize) -> Point {
        Point { row, col }
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
    rows: usize,
    cols: usize,
    content: Vec<Cell>,
}

impl Canvas {
    pub fn new(rows: usize, cols: usize) -> Canvas {
        assert!(rows > 0);
        assert!(cols > 0);
        Canvas {
            rows,
            cols,
            content: vec![Cell::EMPTY; rows * cols],
        }
    }

    pub fn row(&self, row: usize) -> &[Cell] {
        assert!(row < self.rows);
        let start = row * self.cols;
        let end = start + self.cols;
        &self.content[start..end]
    }

    pub fn row_mut(&mut self, row: usize) -> &mut [Cell] {
        assert!(row < self.rows);
        let start = row * self.cols;
        let end = start + self.cols;
        &mut self.content[start..end]
    }

    pub fn put(&mut self, row: usize, col: usize, cell: Cell) {
        assert!(row < self.rows);
        assert!(col < self.cols);
        self.content[row * self.cols + col] = cell;
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
                    Point::new(i / self.cols, i % self.cols),
                    other.content[i],
                ));
                self.content[i] = other.content[i];
            }
        }
        changes
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
    row: usize,
}

pub struct ColIter<'a> {
    canvas: &'a Canvas,
    row_start: usize,
    col: usize,
}

impl<'a> Iterator for RowIter<'a> {
    type Item = (usize, ColIter<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.canvas.rows {
            let row = self.row;
            self.row += 1;
            Some((
                row,
                ColIter {
                    canvas: &self.canvas,
                    row_start: row * self.canvas.cols,
                    col: 0,
                },
            ))
        } else {
            None
        }
    }
}

impl<'a> Iterator for ColIter<'a> {
    type Item = (usize, Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.col < self.canvas.cols {
            let col = self.col;
            self.col += 1;
            Some((col, self.canvas.content[self.row_start + col]))
        } else {
            None
        }
    }
}

pub struct Iter<'a> {
    canvas: &'a Canvas,
    row: usize,
    col: usize,
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
