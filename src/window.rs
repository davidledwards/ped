//! Window management.
use crate::canvas::Canvas;
use crate::color::Color;
use crate::display::{Cell, Display, Point, Size};

use std::cell::RefCell;
use std::cmp;
use std::rc::Rc;

pub struct Window {
    origin: Point,
    size: Size,
    color: Color,
    back: Canvas,
    front: Canvas,
    display: Display,
    blank: Cell,
}

pub type WindowRef = Rc<RefCell<Window>>;

impl Window {
    pub fn new(origin: Point, size: Size, color: Color) -> Window {
        debug_assert!(size.rows > 0);
        debug_assert!(size.cols > 0);

        Window {
            origin,
            size,
            color,
            back: Canvas::new(size),
            front: Canvas::new(size),
            display: Display::new(origin, size),
            blank: Cell::new(' ', color),
        }
    }

    pub fn to_ref(window: Window) -> WindowRef {
        Rc::new(RefCell::new(window))
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
        *self.back.cell_mut(row, col) = cell;
    }

    /// Clears all cells in the column range [`start_col`..`end_col`) for the given `row`.
    pub fn clear_row_range(&mut self, row: u32, start_col: u32, end_col: u32) {
        debug_assert!(row < self.size.rows);
        debug_assert!(start_col < end_col);
        debug_assert!(end_col <= self.size.cols);
        let cells = self.back.row_mut(row);
        cells[(start_col as usize)..(end_col as usize)].fill(self.blank);
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

    /// Scrolls _up_ the contents of the window above `row` by the number of `rows`, and
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

            // Move rows to top of canvas.
            if from_row < row {
                self.back.move_rows(from_row, 0, row - from_row);
            }

            // Clears rows vacated by scroll.
            self.back.clear_rows(row - from_row, row);
        }
    }

    /// Scrolls _down_ the contents of the window at `row` by the number of `rows`, and
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

            // Move rows to bottom of canvas.
            if to_row < self.size.rows {
                self.back.move_rows(row, to_row, self.size.rows - to_row);
            }

            // Clears rows vacated by scroll.
            self.back.clear_rows(row, to_row);
        }
    }

    /// Sets the cursor position in the window to `cursor`.
    pub fn set_cursor(&mut self, cursor: Point) {
        debug_assert!(cursor.row < self.size.rows);
        debug_assert!(cursor.col < self.size.cols);
        self.display.write_cursor(cursor);
        self.display.send();
    }

    /// Draw pending window modifications on display.
    pub fn draw(&mut self) {
        // Determine which cells changed in back canvas, if any, which then results in
        // constructing series of instructions to update display.
        let changes = self.front.reconcile(&self.back);
        if changes.len() > 0 {
            let mut hint = None;
            for (p, cell) in changes {
                self.display.write_cell(p, cell, hint);
                hint = Some((p, cell));
            }
            self.display.send();
        }
    }

    /// Clears the front canvas such that a subsequent [`draw`](Self::draw) will effectively
    /// render the entire display.
    pub fn clear(&mut self) {
        self.front.clear();
    }
}
