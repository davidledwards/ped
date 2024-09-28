//! Grid of cells for arranging text.
use crate::display::{Cell, Point, Size};

pub struct Grid {
    size: Size,
    content: Vec<Cell>,
}

impl Grid {
    pub fn new(size: Size) -> Grid {
        debug_assert!(size.rows > 0);
        debug_assert!(size.cols > 0);
        Grid {
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

    // Apply differences in `other` with respect to this grid and return a vector
    // of those differences.
    //
    // Note that a side effect is that this grid will be equivalent to `other` upon
    // return.
    pub fn reconcile(&mut self, other: &Grid) -> Vec<(Point, Cell)> {
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
            grid: &self,
            row: 0,
            col: 0,
            index: 0,
        }
    }

    pub fn row_iter(&self) -> RowIter<'_> {
        RowIter {
            grid: &self,
            row: 0,
        }
    }
}

pub struct RowIter<'a> {
    grid: &'a Grid,
    row: u32,
}

pub struct ColIter<'a> {
    grid: &'a Grid,
    row_start: usize,
    col: u32,
}

impl<'a> Iterator for RowIter<'a> {
    type Item = (u32, ColIter<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.grid.size.rows {
            let row = self.row;
            self.row += 1;
            Some((
                row,
                ColIter {
                    grid: &self.grid,
                    row_start: (row * self.grid.size.cols) as usize,
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
        if self.col < self.grid.size.cols {
            let col = self.col;
            self.col += 1;
            Some((col, self.grid.content[self.row_start + (col as usize)]))
        } else {
            None
        }
    }
}

pub struct Iter<'a> {
    grid: &'a Grid,
    row: u32,
    col: u32,
    index: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Point, Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.grid.size.rows {
            let item = Some((
                Point::new(self.row, self.col),
                self.grid.content[self.index],
            ));
            self.col += 1;
            if self.col == self.grid.size.cols {
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
