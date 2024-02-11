use std::ops::{Deref, DerefMut};

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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Cell {
    pub value: char,
    pub fg: u8,
    pub bg: u8,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        value: '\0',
        fg: 0,
        bg: 0,
    };

    pub fn new(value: char, fg: u8, bg: u8) -> Cell {
        Cell { value, fg, bg }
    }
}

impl Default for Cell {
    fn default() -> Cell {
        Cell::EMPTY
    }
}

pub struct Canvas {
    rows: usize,
    cols: usize,
    content: Vec<Cell>,
}

// experimental
impl Deref for Canvas {
    type Target = [Cell];

    fn deref(&self) -> &[Cell] {
        &self.content
    }
}

// experimental
impl DerefMut for Canvas {
    fn deref_mut(&mut self) -> &mut [Cell] {
        &mut self.content
    }
}

// operations on the canvas:
// - the window type will be managing the front and back canvas objects.
// - will become more clear when implementing window type.
// - window will write to the back canvas, then diff with the front canvas to determine ANSI
//   commands that should be sent to the terminal.
//
//   fn diff(&self, other: &Canvas) -> Vec<(Point, Cell)>
//   - returns every cell that changed
//   - implement efficient diff as opposed to using iterators
//   - invariant: canvases must have identical dimensions
//
//   window type would then take that diff vector and generate most efficient ANSI sequence,
//   e.g. if two adjacent points in the diff are adjacent on the terminal, then a cursor
//   positioning sequence need not be sent.
//
impl Canvas {
    pub fn new(rows: usize, cols: usize) -> Canvas {
        assert!(rows > 0);
        assert!(cols > 0);
        Canvas {
            rows,
            cols,
            content: vec![Cell::default(); rows * cols],
        }
    }

    pub fn content(&self, row: usize) -> &[Cell] {
        &self.content
    }

    pub fn get_cell(&self, p: &Point) -> &Cell {
        assert!(p.row < self.rows);
        assert!(p.col < self.cols);
        &self.content[p.row * self.cols + p.col]
    }

    pub fn row(&self, row: usize) -> &[Cell] {
        let start = row * self.cols;
        let end = start + self.cols;
        &self.content[start..end]
    }

    pub fn row_mut(&mut self, row: usize) -> &mut [Cell] {
        let start = row * self.cols;
        let end = start + self.cols;
        &mut self.content[start..end]
    }

    pub fn put(&mut self, row: usize, col: usize, cell: Cell) {
        assert!(row < self.rows);
        assert!(col < self.cols);
        self.content[row * self.cols + col] = cell;
    }

    // compare back and front canvases, generate list of changes.
    // specifically, return list of cells from back that differ from front.
    // both self and other are equivalent upon completion.
    //
    // a.reoncile(&b): return cells in "b" that differ from "a"
    //
    // front.reconcile(&back): return cells in "back" that differ from cells in "front"
    //
    pub fn reconcile(&mut self, other: &Canvas) -> Vec<(Point, Cell)> {
        assert!(self.rows == other.rows);
        assert!(self.cols == other.cols);
        let mut changes = Vec::new();
        for i in 0..(self.rows * self.cols) {
            if self.content[i] != other.content[i] {
                changes.push((Point::new(i / self.cols, i % self.cols), other.content[i]));
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
    type Item = (usize, &'a Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.col < self.canvas.cols {
            let col = self.col;
            self.col += 1;
            Some((col, &self.canvas.content[self.row_start + col]))
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
    type Item = (Point, &'a Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.canvas.rows {
            let item = Some((
                Point::new(self.row, self.col),
                &self.canvas.content[self.index],
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
