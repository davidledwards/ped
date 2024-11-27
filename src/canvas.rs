//! Canvas.
use crate::grid::{Cell, Grid};
use crate::size::{Point, Size};
use crate::writer::Writer;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Canvas {
    size: Size,
    back: Grid,
    front: Grid,
    writer: Writer,
}

pub type CanvasRef = Rc<RefCell<Canvas>>;

impl Canvas {
    pub fn new(origin: Point, size: Size) -> Canvas {
        Canvas {
            size,
            back: Grid::new(size),
            front: Grid::new(size),
            writer: Writer::new(origin),
        }
    }

    pub fn zero() -> Canvas {
        Canvas {
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

    pub fn size(&self) -> Size {
        self.size
    }

    /// Set value of \[`row`, `col`\] to `cell`.
    pub fn set_cell(&mut self, row: u32, col: u32, cell: Cell) {
        debug_assert!(row < self.size.rows);
        debug_assert!(col < self.size.cols);

        self.back.set_cell(row, col, cell);
    }

    /// Fills all cells with `cell` in the column range [`start_col`, `end_col`)
    /// for the given `row`.
    pub fn fill_row_range(&mut self, row: u32, start_col: u32, end_col: u32, cell: Cell) {
        debug_assert!(row < self.size.rows);
        debug_assert!(start_col <= end_col);
        debug_assert!(end_col <= self.size.cols);

        self.back.fill_range(row, start_col, end_col, cell);
    }

    /// Fills all cells with `cell` in the column range [`start_col`, ..) for the
    /// given `row`.
    pub fn fill_row_from(&mut self, row: u32, start_col: u32, cell: Cell) {
        self.fill_row_range(row, start_col, self.size.cols, cell);
    }

    /// Fills all cells with `cell` for the given `row`.
    #[allow(dead_code)]
    pub fn fill_row(&mut self, row: u32, cell: Cell) {
        self.fill_row_from(row, 0, cell);
    }

    /// Fills all cells with `cell` for rows in the range [`start_row`, `end_row`).
    #[allow(dead_code)]
    pub fn fill_rows(&mut self, start_row: u32, end_row: u32, cell: Cell) {
        for row in start_row..end_row {
            self.fill_row(row, cell);
        }
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
