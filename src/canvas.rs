//! An abstraction over the terminal display that represents the visible area as a
//! grid of cells.
//!
//! A canvas is comprised of a _front_ and _back_ grid, where the front faithfully
//! represents what is visible on the display and the back contains pending updates
//! not yet visible.
//!
//! Updates to the canvas are always written to the _back_ grid first, then reconciled
//! with the _front_ grid to essentially produce an intermediate diff, which is then
//! used to generate the terminal output.

use crate::color::Color;
use crate::grid::{Cell, Grid};
use crate::size::{Point, Size};
use crate::writer::Writer;
use std::cell::RefCell;
use std::cmp;
use std::ops::Range;
use std::rc::Rc;

/// An abstraction over the terminal display.
///s
/// A canvas is defined by its _origin_, which is relative to the top-left corner of
/// the display, and its _size_, which is the number of rows and columns.
pub struct Canvas {
    origin: Point,
    size: Size,
    back: Grid,
    front: Grid,
    writer: Writer,
}

pub type CanvasRef = Rc<RefCell<Canvas>>;

impl Canvas {
    pub fn new(origin: Point, size: Size) -> Canvas {
        Canvas {
            origin,
            size,
            back: Grid::new(size),
            front: Grid::new(size),
            writer: Writer::new(origin),
        }
    }

    pub fn zero() -> Canvas {
        Canvas {
            origin: Point::ORIGIN,
            size: Size::ZERO,
            back: Grid::zero(),
            front: Grid::zero(),
            writer: Writer::new(Point::ORIGIN),
        }
    }

    /// Turns the canvas into a [`CanvasRef`].
    pub fn to_ref(self) -> CanvasRef {
        Rc::new(RefCell::new(self))
    }

    pub fn origin(&self) -> Point {
        self.origin
    }

    pub fn size(&self) -> Size {
        self.size
    }

    /// Sets the cell at (`row`, `col`) to the value `cell`.
    pub fn set_cell(&mut self, row: u32, col: u32, cell: Cell) {
        debug_assert!(row < self.size.rows);
        debug_assert!(col < self.size.cols);
        self.back.set_cell(row, col, cell);
    }

    /// Sets the cell at (`row`, `col`) to the value `c` using `color`.
    #[inline]
    pub fn set(&mut self, row: u32, col: u32, c: char, color: Color) {
        self.set_cell(row, col, Cell::new(c, color));
    }

    /// Writes `c` using `color` at (`row`, `col`), returning `1` if the character
    /// was written or `0` if `col` extends beyond the end of the row.
    pub fn write_char(&mut self, row: u32, col: u32, c: char, color: Color) -> u32 {
        if col < self.size.cols {
            self.back.set_cell(row, col, Cell::new(c, color));
            1
        } else {
            0
        }
    }

    /// Write `chars` using `color` in `row` starting at column `col`, returning the
    /// actual number of characters written.
    ///
    /// Fewer than `chars.len()` may be written if doing so would extend beyond the
    /// end of the row.
    pub fn write(&mut self, row: u32, col: u32, chars: &[char], color: Color) -> u32 {
        debug_assert!(row < self.size.rows);
        let n = cmp::min(chars.len(), (self.size.cols - col) as usize);
        for (i, c) in chars.iter().take(n).enumerate() {
            self.back
                .set_cell(row, col + i as u32, Cell::new(*c, color));
        }
        n as u32
    }

    /// Write `chars` using `color` in `row` starting at column `col`, returning the
    /// actual number of characters written.
    pub fn write_str(&mut self, row: u32, col: u32, chars: &str, color: Color) -> u32 {
        let chars = chars.chars().collect::<Vec<_>>();
        self.write(row, col, &chars, color)
    }

    /// Fills cells in `row` for the column range `cols` with the value `cell`.
    pub fn fill_cell(&mut self, row: u32, cols: Range<u32>, cell: Cell) {
        debug_assert!(row < self.size.rows);
        let Range { start, end } = cols;
        debug_assert!(start <= end);
        debug_assert!(end <= self.size.cols);
        self.back.fill_range(row, start, end, cell);
    }

    /// Fills cells in `row` for the column range `cols` with the value `c` using
    /// `color`.
    #[inline]
    pub fn fill(&mut self, row: u32, cols: Range<u32>, c: char, color: Color) {
        self.fill_cell(row, cols, Cell::new(c, color));
    }

    /// Fills cells in `row` starting at column `start_col` with the value `cell`.
    #[inline]
    pub fn fill_cell_from(&mut self, row: u32, start_col: u32, cell: Cell) {
        self.fill_cell(row, start_col..self.size.cols, cell);
    }

    /// Fills cells in `row` starting at column `start_col` with the value `c` using
    /// `color`.
    #[inline]
    pub fn fill_from(&mut self, row: u32, start_col: u32, c: char, color: Color) {
        self.fill_cell_from(row, start_col, Cell::new(c, color));
    }

    /// Fills all cells in `row` with the value `c` using `color`.
    #[inline]
    pub fn fill_row(&mut self, row: u32, c: char, color: Color) {
        self.fill(row, 0..self.size.cols, c, color);
    }

    /// Sets the cursor position on the canvas to `cursor`.
    pub fn set_cursor(&mut self, cursor: Point) {
        debug_assert!(cursor.row < self.size.rows);
        debug_assert!(cursor.col < self.size.cols);
        self.writer.set_cursor(cursor);
        self.writer.send();
    }

    /// Draw pending canvas modifications.
    pub fn draw(&mut self) {
        // Determine which cells changed in back grid, if any, which then results in
        // constructing series of instructions to update display.
        let changes = self.front.reconcile(&self.back);
        if changes.len() > 0 {
            let mut hint = None;
            self.writer.hide_cursor();
            for (p, cell) in changes {
                self.draw_cell(p, cell, hint);
                hint = Some((p, cell));
            }
            self.writer.show_cursor().send();
        }
    }

    /// Clears the front grid such that a subsequent [`draw`](Self::draw) will effectively
    /// render the entire canvas.
    pub fn clear(&mut self) {
        self.front.clear();
    }

    /// Draws `cell` at point `p`.
    ///
    /// An optional `hint` is used to optimize the output, where the hint is the last
    /// cell drawn.
    fn draw_cell(&mut self, p: Point, cell: Cell, hint: Option<(Point, Cell)>) {
        match hint {
            Some((prev_p, prev_cell)) => {
                if p.row != prev_p.row || p.col != prev_p.col + 1 {
                    self.writer.set_cursor(p);
                }
                if cell.color != prev_cell.color {
                    self.writer.set_color(cell.color);
                }
            }
            None => {
                self.writer.set_cursor(p).set_color(cell.color);
            }
        }
        self.writer.write(cell.value);
    }
}
