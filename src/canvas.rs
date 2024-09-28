//! Canvas.
use crate::color::Color;
use crate::display::{Display, Point, Size};
use crate::grid::Cell;
use crate::grid::Grid;

use std::cell::RefCell;
use std::cmp;
use std::rc::Rc;

pub struct Canvas {
    origin: Point,
    size: Size,
    color: Color,
    back: Grid,
    front: Grid,
    display: Display,
    blank: Cell,
}

pub type CanvasRef = Rc<RefCell<Canvas>>;

impl Canvas {
    pub fn new(origin: Point, size: Size, color: Color) -> Canvas {
        Canvas {
            origin,
            size,
            color,
            back: Grid::new(size),
            front: Grid::new(size),
            display: Display::new(origin),
            blank: Cell::new(' ', color),
        }
    }

    /// Turns `canvas` into a [`CanvasRef`].
    pub fn to_ref(canvas: Canvas) -> CanvasRef {
        Rc::new(RefCell::new(canvas))
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn color(&self) -> Color {
        self.color
    }

    /// Set value of \[`row`, `col`\] to `cell`.
    pub fn set_cell(&mut self, row: u32, col: u32, cell: Cell) {
        debug_assert!(row < self.size.rows);
        debug_assert!(col < self.size.cols);

        self.back.set_cell(row, col, cell);
    }

    /// Clears all cells in the column range [`start_col`..`end_col`) for the given `row`.
    pub fn clear_row_range(&mut self, row: u32, start_col: u32, end_col: u32) {
        debug_assert!(row < self.size.rows);
        debug_assert!(start_col < end_col);
        debug_assert!(end_col <= self.size.cols);

        self.back.fill_range(row, start_col, end_col, self.blank);
    }

    /// Clears all cells in the column range [`start_col`..) for the given `row`.
    pub fn clear_row_from(&mut self, row: u32, start_col: u32) {
        self.clear_row_range(row, start_col, self.size.cols);
    }

    /// Clears all cells for the given `row`.
    pub fn clear_row(&mut self, row: u32) {
        self.clear_row_from(row, 0);
    }

    /// Clears all cells for rows in the range [`start_row`..`end_row`).
    pub fn clear_rows(&mut self, start_row: u32, end_row: u32) {
        for row in start_row..end_row {
            self.clear_row(row);
        }
    }

    /// Scrolls _up_ the contents of the canvas above `row` by the number of `rows`, and
    /// clears the vacated rows.
    ///
    /// # Example
    /// Given the following contents indexed by row:
    ///
    /// ```text
    ///    -----
    /// 0 |the
    /// 1 |quick
    /// 2 |brown
    /// 3 |fox
    /// 4 |jumps
    ///    -----
    /// ```
    ///
    /// `scroll_up(3, 2)` yields:
    ///
    /// ```text
    ///    -----
    /// 0 |brown
    /// 1 |
    /// 2 |
    /// 3 |fox
    /// 4 |jumps
    ///    -----
    /// ```
    pub fn scroll_up(&mut self, row: u32, rows: u32) {
        debug_assert!(row <= self.size.rows);

        if rows > 0 {
            // Start row of move is maximally bounded by number of rows to scroll.
            let from_row = cmp::min(rows, row);

            // Move rows to top of grid.
            if from_row < row {
                self.back.move_rows(from_row, 0, row - from_row);
            }

            // Clears rows vacated by scroll.
            self.back.clear_rows(row - from_row, row);
        }
    }

    /// Scrolls _down_ the contents of the canvas at `row` by the number of `rows`, and
    /// clears the vacated rows.
    ///
    /// # Example
    /// Given the following contents indexed by row:
    ///
    /// ```text
    ///    -----
    /// 0 |the
    /// 1 |quick
    /// 2 |brown
    /// 3 |fox
    /// 4 |jumps
    ///    -----
    /// ```
    ///
    /// `scroll_down(1, 2)` yields:
    ///
    /// ```text
    ///    -----
    /// 0 |the
    /// 1 |
    /// 2 |
    /// 3 |quick
    /// 4 |brown
    ///    -----
    /// ```
    pub fn scroll_down(&mut self, row: u32, rows: u32) {
        debug_assert!(row < self.size.rows);

        if rows > 0 {
            // Target row of move is maximally bounded by total number of rows.
            let to_row = cmp::min(row + rows, self.size.rows);

            // Move rows to bottom of grid.
            if to_row < self.size.rows {
                self.back.move_rows(row, to_row, self.size.rows - to_row);
            }

            // Clears rows vacated by scroll.
            self.back.clear_rows(row, to_row);
        }
    }

    /// Sets the cursor position on the canvas to `cursor`.
    pub fn set_cursor(&mut self, cursor: Point) {
        debug_assert!(cursor.row < self.size.rows);
        debug_assert!(cursor.col < self.size.cols);

        self.display.set_cursor(cursor);
        self.display.send();
    }

    /// Draw pending canvas modifications to display.
    pub fn draw(&mut self) {
        // Determine which cells changed in back grid, if any, which then results in
        // constructing series of instructions to update display.
        let changes = self.front.reconcile(&self.back);
        if changes.len() > 0 {
            let mut hint = None;
            for (p, cell) in changes {
                self.draw_cell(p, cell, hint);
                hint = Some((p, cell));
            }
            self.display.send();
        }
    }

    /// Clears the front grid such that a subsequent [`draw`](Self::draw) will effectively
    /// render the entire display.
    pub fn clear(&mut self) {
        self.front.clear();
    }

    /// Draws `cell` at point `p`.
    ///
    /// An optional `hint` is used optimize display output, where the hint is the last
    /// cell drawn.
    fn draw_cell(&mut self, p: Point, cell: Cell, hint: Option<(Point, Cell)>) {
        match hint {
            Some((prev_p, prev_cell)) => {
                if p.row != prev_p.row || p.col != prev_p.col + 1 {
                    self.display.set_cursor(p);
                }
                if cell.color != prev_cell.color {
                    self.display.set_color(cell.color);
                }
            }
            None => {
                self.display.set_cursor(p).set_color(cell.color);
            }
        }
        self.display.write(cell.value);
    }
}
