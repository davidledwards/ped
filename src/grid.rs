//! A grid of cells representing characters on a display.

use crate::color::Color;
use crate::size::{Point, Size};

/// A 2-dimensional array of [`Cell`]s.
pub struct Grid {
    size: Size,
    content: Vec<Cell>,
}

/// A pair containing a [`char`] and a [`Color`].
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Cell {
    pub value: char,
    pub color: Color,
}

impl Cell {
    pub const EMPTY: Cell = Cell::new('\0', Color::ZERO);

    pub const fn new(value: char, color: Color) -> Cell {
        Cell { value, color }
    }
}

impl Default for Cell {
    fn default() -> Cell {
        Cell::EMPTY
    }
}

impl Grid {
    /// Creates a `size` grid with all cells set to [`Cell::EMPTY`].
    pub fn new(size: Size) -> Grid {
        Grid {
            size,
            content: vec![Cell::EMPTY; (size.rows * size.cols) as usize],
        }
    }

    /// Creats a grid of size [`Size::ZERO`].
    pub fn zero() -> Grid {
        Grid {
            size: Size::ZERO,
            content: Vec::new(),
        }
    }

    /// Sets the `row`:`col` cell to `cell`.
    pub fn set_cell(&mut self, row: u32, col: u32, cell: Cell) {
        debug_assert!(row < self.size.rows);
        debug_assert!(col < self.size.cols);

        self.content[(row * self.size.cols + col) as usize] = cell;
    }

    /// Fills all cells in `row` in the range [`start_col`, `end_col`) to `cell`.
    pub fn fill_range(&mut self, row: u32, start_col: u32, end_col: u32, cell: Cell) {
        debug_assert!(row < self.size.rows);
        debug_assert!(start_col <= end_col);
        debug_assert!(end_col <= self.size.cols);

        let row_index = row * self.size.cols;
        let start = (row_index + start_col) as usize;
        let end = (row_index + end_col) as usize;
        self.content[start..end].fill(cell);
    }

    /// Sets all cells to [`Cell::EMPTY`].
    pub fn clear(&mut self) {
        self.content.fill(Cell::EMPTY);
    }

    /// Apply differences in `other` grid with respect to this grid and return a vector
    /// of those differences.
    ///
    /// Note that a side effect is that this grid will be equivalent to `other` upon
    /// return.
    ///
    /// Both grids must have equivalent sizes, otherwise the function panics.
    pub fn reconcile(&mut self, other: &Grid) -> Vec<(Point, Cell)> {
        debug_assert!(self.size.rows == other.size.rows);
        debug_assert!(self.size.cols == other.size.cols);

        self.content
            .iter_mut()
            .enumerate()
            .filter_map(|(i, this_cell)| {
                let other_cell = other.content[i];
                if *this_cell != other_cell {
                    *this_cell = other_cell;
                    let row = (i as u32) / self.size.cols;
                    let col = (i as u32) % self.size.cols;
                    Some((Point::new(row, col), other_cell))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }
}
