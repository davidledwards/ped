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
    /// Creates a grid of the given `size` with all cells set to [`Cell::EMPTY`].
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
    /// Note that an important side effect of this method is that this grid will be
    /// equivalent to `other` upon return.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::zip;

    const GRID_ROWS: u32 = 3;
    const GRID_COLS: u32 = 3;
    const GRID_SIZE: Size = Size::new(GRID_ROWS, GRID_COLS);

    const ZIG_CONTENT: [char; (GRID_ROWS * GRID_COLS) as usize] =
        ['z', 'i', 'g', 'Z', 'i', 'g', 'z', 'I', 'g'];

    const ZAG_CONTENT: [char; (GRID_ROWS * GRID_COLS) as usize] =
        ['z', 'i', 'G', 'z', 'i', 'g', 'z', 'i', 'g'];

    const ZIG_ZAG_DIFF: [(Point, char); 3] = [
        (Point::new(0, 2), 'G'),
        (Point::new(1, 0), 'z'),
        (Point::new(2, 1), 'i'),
    ];

    #[test]
    fn new_grid() {
        let grid = empty_grid();
        assert_eq!(grid.size, GRID_SIZE);
        assert_eq!(
            grid.content,
            vec![Cell::EMPTY; (GRID_SIZE.rows * GRID_SIZE.cols) as usize]
        )
    }

    #[test]
    fn new_zero_grid() {
        let grid = Grid::zero();
        assert_eq!(grid.size, Size::ZERO);
        assert_eq!(grid.content.len(), 0);
    }

    #[test]
    fn reconcile_equivalent_grids() {
        let mut this_grid = zig_grid();
        let that_grid = zig_grid();

        // Should be no differences.
        let diff = this_grid.reconcile(&that_grid);
        assert_eq!(diff.len(), 0);
    }

    #[test]
    fn reconcile_different_grids() {
        let mut this_grid = zig_grid();
        let that_grid = zag_grid();

        // Verify differences.
        let diff = this_grid.reconcile(&that_grid);
        assert_eq!(diff.len(), ZIG_ZAG_DIFF.len());
        for ((p, cell), (diff_p, diff_c)) in zip(diff, ZIG_ZAG_DIFF) {
            assert_eq!(p, diff_p);
            assert_eq!(cell.value, diff_c);
        }

        // Verify that both grids are now equivalent.
        assert_eq!(this_grid.content, that_grid.content);
    }

    fn empty_grid() -> Grid {
        Grid::new(GRID_SIZE)
    }

    fn zig_grid() -> Grid {
        let mut grid = Grid::new(GRID_SIZE);
        for i in 0..ZIG_CONTENT.len() {
            grid.content[i] = Cell::new(ZIG_CONTENT[i], Color::ZERO);
        }
        grid
    }

    fn zag_grid() -> Grid {
        let mut grid = Grid::new(GRID_SIZE);
        for i in 0..ZAG_CONTENT.len() {
            grid.content[i] = Cell::new(ZAG_CONTENT[i], Color::ZERO);
        }
        grid
    }
}
