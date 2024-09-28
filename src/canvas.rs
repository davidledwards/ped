//! Represents visible content of buffers.
use crate::display::{Cell, Point, Size};

#[derive(Debug)]
pub struct Canvas {
    size: Size,
    content: Vec<Cell>,
}

impl Canvas {
    pub fn new(size: Size) -> Canvas {
        debug_assert!(size.rows > 0);
        debug_assert!(size.cols > 0);
        Canvas {
            size,
            content: vec![Cell::EMPTY; (size.rows * size.cols) as usize],
        }
    }

    pub fn row(&self, row: u32) -> &[Cell] {
        debug_assert!(row < self.size.rows);
        let start = row * self.size.cols;
        let end = start + self.size.cols;
        &self.content[(start as usize)..(end as usize)]
    }

    pub fn row_mut(&mut self, row: u32) -> &mut [Cell] {
        debug_assert!(row < self.size.rows);
        let start = row * self.size.cols;
        let end = start + self.size.cols;
        &mut self.content[(start as usize)..(end as usize)]
    }

    pub fn cell_mut(&mut self, row: u32, col: u32) -> &mut Cell {
        debug_assert!(row < self.size.rows);
        debug_assert!(col < self.size.cols);
        &mut self.content[(row * self.size.cols + col) as usize]
    }

    pub fn clear(&mut self) {
        self.content.fill(Cell::EMPTY);
    }

    // Apply differences in other with respect to this canvas and return a vector of
    // those differences.
    //
    // Note that this canvas will be equivalent to other upon return.
    pub fn reconcile(&mut self, other: &Canvas) -> Vec<(Point, Cell)> {
        debug_assert!(self.size.rows == other.size.rows);
        debug_assert!(self.size.cols == other.size.cols);

        let mut changes = Vec::new();
        for i in 0..self.content.len() {
            if self.content[i] != other.content[i] {
                changes.push((
                    Point::new((i as u32) / self.size.cols, (i as u32) % self.size.cols),
                    other.content[i],
                ));
                self.content[i] = other.content[i];
            }
        }
        changes
    }

    pub fn move_rows(&mut self, from_row: u32, to_row: u32, rows: u32) {
        debug_assert!(from_row < self.size.rows);
        debug_assert!(to_row < self.size.rows);
        debug_assert!(if from_row < to_row {
            to_row + rows <= self.size.rows
        } else {
            from_row + rows <= self.size.rows
        });

        let start = (from_row * self.size.cols) as usize;
        let end = start + (rows * self.size.cols) as usize;
        let dest = (to_row * self.size.cols) as usize;
        self.content.copy_within(start..end, dest);
    }

    pub fn clear_rows(&mut self, start_row: u32, end_row: u32) {
        debug_assert!(start_row < self.size.rows);
        debug_assert!(end_row <= self.size.rows);
        debug_assert!(start_row <= end_row);

        let start = (start_row * self.size.cols) as usize;
        let end = (end_row * self.size.cols) as usize;
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
        if self.row < self.canvas.size.rows {
            let row = self.row;
            self.row += 1;
            Some((
                row,
                ColIter {
                    canvas: &self.canvas,
                    row_start: (row * self.canvas.size.cols) as usize,
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
        if self.col < self.canvas.size.cols {
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
        if self.row < self.canvas.size.rows {
            let item = Some((
                Point::new(self.row, self.col),
                self.canvas.content[self.index],
            ));
            self.col += 1;
            if self.col == self.canvas.size.cols {
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
