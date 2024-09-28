//! Grid of cells for arranging characters.
use crate::color::Color;
use crate::display::{Point, Size};

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

    /// Sets the `row`:`col` cell to `cell`.
    pub fn set_cell(&mut self, row: u32, col: u32, cell: Cell) {
        debug_assert!(row < self.size.rows);
        debug_assert!(col < self.size.cols);

        self.content[(row * self.size.cols + col) as usize] = cell;
    }

    /// Fills all cells in `row` in the range [`start_col`, `end_col`) to `cell`.
    pub fn fill_range(&mut self, row: u32, start_col: u32, end_col: u32, cell: Cell) {
        debug_assert!(row < self.size.rows);
        debug_assert!(start_col < end_col);
        debug_assert!(end_col <= self.size.cols);

        let row_index = row * self.size.cols;
        let start = (row_index + start_col) as usize;
        let end = (row_index + end_col) as usize;
        self.content[start..end].fill(cell);
    }

    /// Moves `rows` rows starting from `from_row` to `to_row`.
    ///
    /// This function is safe to move overlapping rows. However, if the range of rows
    /// relative to either `from_row` or `to_row` would extend beyond the grid size, then
    /// this function will panic.
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

    /// Sets all cells to [`Cell::EMPTY`].
    pub fn clear(&mut self) {
        self.content.fill(Cell::EMPTY);
    }

    /// Sets the cells of all rows in the range [`start_row`, `end_row`) to
    /// [`Cell::EMPTY`].
    pub fn clear_rows(&mut self, start_row: u32, end_row: u32) {
        debug_assert!(start_row < self.size.rows);
        debug_assert!(end_row <= self.size.rows);
        debug_assert!(start_row <= end_row);

        let start = (start_row * self.size.cols) as usize;
        let end = (end_row * self.size.cols) as usize;
        self.content[start..end].fill(Cell::EMPTY);
    }

    // Apply differences in `other` grid with respect to this grid and return a vector
    // of those differences.
    //
    // Note that a side effect is that this grid will be equivalent to `other` upon
    // return.
    //
    // Both grids must have equivalent sizes, otherwise the function panics.
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
