//! Editor.
use crate::buffer::{Buffer, BufferRef};
use crate::canvas::{Canvas, CanvasRef};
use crate::display::{Point, Size};
use crate::grid::Cell;
use crate::window::{Window, WindowRef};

use std::cell::{Ref, RefCell, RefMut};
use std::cmp;
use std::rc::Rc;

pub struct Editor {
    /// Buffer containing the contents of this editor.
    buffer: BufferRef,

    /// Buffer position corresponding to the cursor.
    cur_pos: usize,

    /// Window attached to this editor.
    window: WindowRef,

    canvas: CanvasRef,

    /// Cached value of canvas size.
    size: Size,

    /// Buffer position corresponding to the first character of the `cursor` row.
    row_pos: usize,

    /// Position of the cursor in `window`.
    cursor: Point,
}

pub type EditorRef = Rc<RefCell<Editor>>;

/// Various cursor alignment directives.
pub enum Align {
    /// Try using the current cursor position captured by [`Editor::cursor`].
    Auto,

    /// Try aligning the cursor in the center of the window.
    Center,

    /// Try aligning the cursor to a specific row.
    Row(u32),
}

impl Editor {
    pub fn new(buffer: BufferRef) -> Editor {
        let cur_pos = buffer.borrow().get_pos();
        let window = Window::zombie().to_ref();
        let canvas = window.borrow().canvas().clone();
        let size = canvas.borrow().size();

        Editor {
            buffer,
            cur_pos,
            window,
            canvas,
            size,
            row_pos: 0,
            cursor: Point::ORIGIN,
        }
    }

    /// Turns the editor into a [`EditorRef`].
    pub fn to_ref(self: Editor) -> EditorRef {
        Rc::new(RefCell::new(self))
    }

    pub fn attach(&mut self, window: WindowRef) {
        self.window = window;
        self.canvas = self.window.borrow().canvas().clone();
        self.size = self.canvas.borrow().size();

        if !self.window.borrow().is_zombie() {
            self.align_cursor(Align::Auto);
            self.draw();
        }
    }

    pub fn draw(&mut self) {
        let (top_pos, _) = self.find_up(self.row_pos, self.cursor.row);
        self.render_rows_from(0, top_pos);
        self.canvas_mut().clear();
        self.canvas_mut().draw();
        self.canvas_mut().set_cursor(self.cursor);
    }

    pub fn cursor(&self) -> (Point, usize) {
        (self.cursor, self.cur_pos)
    }

    fn buffer(&self) -> Ref<'_, Buffer> {
        self.buffer.borrow()
    }

    fn buffer_mut(&self) -> RefMut<'_, Buffer> {
        self.buffer.borrow_mut()
    }

    fn canvas(&self) -> Ref<'_, Canvas> {
        self.canvas.borrow()
    }

    fn canvas_mut(&self) -> RefMut<'_, Canvas> {
        self.canvas.borrow_mut()
    }

    pub fn insert_char(&mut self, c: char) {
        self.insert_chars(&vec![c])
    }

    pub fn insert_chars(&mut self, cs: &Vec<char>) {
        // Knowing number of wrapping rows prior to insertion helps optimize rendering
        // when resulting cursor remains on same row.
        let wrap_rows = self.wrapped_rows(self.row_pos);
        self.buffer_mut().set_pos(self.cur_pos);
        let cur_pos = self.buffer_mut().insert_chars(cs);

        // Locate resulting cursor position and render accordingly.
        let (maybe_row, col, row_pos) = self.find_cursor(cur_pos);
        let row = match maybe_row {
            Some(row) => {
                if row == self.cursor.row {
                    if self.wrapped_rows(self.row_pos) > wrap_rows {
                        // Insertion caused more rows to wrap, so everything below current
                        // row essentially gets shifted down.
                        self.render_rows_from(row, self.row_pos);
                    } else {
                        // Number of wrapping rows did not change after insertion, so limit
                        // rendering to only those rows that wrap. Note that number of
                        // wrapping rows could extend beyond bottom of window, so ensure
                        // that number is appropriately bounded.
                        let rows = cmp::min(wrap_rows + 1, self.size.rows - row);
                        self.render_rows(row, rows, self.row_pos);
                    }
                    row
                } else {
                    // New cursor on different row but still visible, so render all rows
                    // starting from current row.
                    self.render_rows_from(self.cursor.row, self.row_pos);
                    row
                }
            }
            None => {
                // New cursor position not visible, so find top of buffer and render entire
                // window.
                let (top_pos, _) = self.find_up(row_pos, self.size.rows - 1);
                self.render_rows_from(0, top_pos);
                self.size.rows - 1
            }
        };
        self.canvas_mut().draw();

        self.cur_pos = cur_pos;
        self.row_pos = row_pos;
        self.set_cursor(row, col);
    }

    pub fn delete_left(&mut self) -> Option<char> {
        if self.cur_pos > 0 {
            let cs = self.remove_from(self.cur_pos - 1);
            Some(cs[0])
        } else {
            None
        }
    }

    pub fn delete_right(&mut self) -> Option<char> {
        if self.cur_pos < self.buffer().size() {
            let cs = self.remove_to(self.cur_pos + 1);
            Some(cs[0])
        } else {
            None
        }
    }

    /// Removes and returns characters from `from_pos` to the cursor position.
    ///
    /// Specifically, characters in the range [`from_pos`, `self.cur_pos`) are removed
    /// if `from_pos` is less than `self.cur_pos`, otherwise the operation is ignored
    /// and an empty vector is returned.
    pub fn remove_from(&mut self, from_pos: usize) -> Vec<char> {
        if from_pos < self.cur_pos {
            // Calculate number of wrapping rows prior to text removal in order to
            // help optimize rendering when resulting cursor remains on same row.
            let wrap_rows = self.wrapped_rows(self.find_row_pos(from_pos));

            // Locate resulting cursor position before removing text.
            let (maybe_row, col, row_pos) = self.find_cursor(from_pos);

            // This unwrap should never panic since condition of entering this block
            // is that > 0 characters will be removed.
            self.buffer_mut().set_pos(from_pos);
            let text = self
                .buffer_mut()
                .remove_chars(self.cur_pos - from_pos)
                .unwrap();

            let row = match maybe_row {
                Some(row) => {
                    if row == self.cursor.row {
                        if self.wrapped_rows(row_pos) < wrap_rows {
                            // Removal caused less rows to wrap, so everything below
                            // current row essentially gets shifted up.
                            self.render_rows_from(row, row_pos);
                        } else {
                            // Number of wrapping rows did not change after removal, so
                            // limit rendering to only those rows that wrap. Note that number
                            // of wrapping rows could extend beyond bottom of window, so
                            // ensure that number is appropriately bounded.
                            let rows = cmp::min(wrap_rows + 1, self.size.rows - row);
                            self.render_rows(row, rows, row_pos);
                        }
                        row
                    } else {
                        // New cursor on different row but still visible, so render all rows
                        // starting from the new row.
                        self.render_rows_from(row, row_pos);
                        row
                    }
                }
                None => {
                    // New cursor position not visible, so position new row at top of
                    // window and render entire window.
                    self.render_rows_from(0, row_pos);
                    0
                }
            };
            self.canvas_mut().draw();
            self.cur_pos = from_pos;
            self.row_pos = row_pos;
            self.set_cursor(row, col);
            text
        } else {
            vec![]
        }
    }

    /// Removes and returns characters from the cursor position to `to_pos`.
    ///
    /// Specifically, characters in the range [`self.cur_pos`, `to_pos`) are removed
    /// if `to_pos` is greater than `self.cur_pos`, otherwise the operation is ignored
    /// and an empty vector is returned.
    pub fn remove_to(&mut self, to_pos: usize) -> Vec<char> {
        let to_pos = cmp::min(to_pos, self.buffer().size());
        if to_pos > self.cur_pos {
            // Possibly capture number of wrapping rows prior to text removal, but only
            // if from and to positions share same beginning of line.
            let wrap_rows = if self.buffer().find_beg_line(to_pos) <= self.cur_pos {
                Some(self.wrapped_rows(self.row_pos))
            } else {
                None
            };

            // This unwrap should never panic since condition of entering this block
            // is that > 0 characters will be removed.
            self.buffer_mut().set_pos(self.cur_pos);
            let text = self
                .buffer_mut()
                .remove_chars(to_pos - self.cur_pos)
                .unwrap();

            match wrap_rows {
                Some(wrap_rows) => {
                    // This condition occurs when from and to positions share same
                    // beginning of line, which means rendering can be optimized under
                    // certain circumstances.
                    if self.wrapped_rows(self.row_pos) < wrap_rows {
                        // Removal caused less rows to wrap, so everything below current
                        // row essentially gets shifted up.
                        self.render_rows_from(self.cursor.row, self.row_pos);
                    } else {
                        // Number of wrapping rows did not change after removal, so limit
                        // rendering to only those rows that wrap. Note that number of
                        // wrapping rows could extend beyond bottom of window, so ensure
                        // that number is appropriately bounded.
                        let rows = cmp::min(wrap_rows + 1, self.size.rows - self.cursor.row);
                        self.render_rows(self.cursor.row, rows, self.row_pos);
                    }
                }
                None => {
                    // This condition occurs when from and to positions do not share
                    // same beginning of line, which means following rows always shift up.
                    self.render_rows_from(self.cursor.row, self.row_pos);
                }
            }
            self.canvas_mut().draw();
            self.canvas_mut().set_cursor(self.cursor);
            text
        } else {
            vec![]
        }
    }

    pub fn move_to(&mut self, pos: usize) {
        // move cursor to given pos
        // set row, col, row_pos accordingly
        // render screen
        // could move up or down
    }

    pub fn align_cursor(&mut self, align: Align) {
        // Determine ideal row where cursor would like to be focused, though this should
        // be considered a hint.
        let row = match align {
            Align::Auto => cmp::min(self.cursor.row, self.size.rows - 1),
            Align::Center => self.size.rows / 2,
            Align::Row(row) => cmp::min(row, self.size.rows - 1),
        };

        // Tries to position cursor on target row, but no guarantee depending on proximity
        // of row to top of buffer.
        let col = self.column_of(self.cur_pos);
        self.row_pos = self.cur_pos - col as usize;
        let (_, row) = self.find_up(self.row_pos, row);
        self.cursor = Point::new(row, col);
    }

    pub fn move_up(&mut self) {
        // Tries to move cursor up by 1 row, though it may already be at top of buffer.
        let (row_pos, rows) = self.find_up(self.row_pos, 1);
        if rows > 0 {
            let (cur_pos, col) = self.find_col(row_pos, self.cursor.col);

            // Changes to canvas only occur when cursor is already at top row of window,
            // otherwise just change position of cursor on display.
            let row = if self.cursor.row > 0 {
                self.cursor.row - 1
            } else {
                self.canvas_mut().scroll_down(0, 1);
                self.render_rows(0, 1, row_pos);
                self.canvas_mut().draw();
                0
            };

            self.cur_pos = cur_pos;
            self.row_pos = row_pos;
            self.set_cursor(row, col);
        }
    }

    pub fn move_down(&mut self) {
        // Tries to move cursor down by 1 row, though it may already be at end of buffer.
        let (row_pos, rows) = self.find_down(self.row_pos, 1);
        if rows > 0 {
            let (cur_pos, col) = self.find_col(row_pos, self.cursor.col);

            // Changes to canvas only occur when cursor is already at bottow row of
            // window, otherwise just change position of cursor on display.
            let row = if self.cursor.row < self.size.rows - 1 {
                self.cursor.row + 1
            } else {
                self.canvas_mut().scroll_up(self.size.rows, 1);
                self.render_rows(self.cursor.row, 1, row_pos);
                self.canvas_mut().draw();
                self.cursor.row
            };

            self.cur_pos = cur_pos;
            self.row_pos = row_pos;
            self.set_cursor(row, col);
        }
    }

    pub fn move_left(&mut self) {
        // Tries to move buffer position left by 1 character, though it may already be
        // at beginning of buffer.
        let left = self.buffer().backward(self.cur_pos).index().next();

        if let Some((cur_pos, c)) = left {
            let (row, col) = if self.cursor.col > 0 {
                // Cursor not at left edge of window, so just a simple cursor move.
                (self.cursor.row, self.cursor.col - 1)
            } else {
                // Cursor at left edge of window, so determine position of prior row
                // and column number.
                let (row_pos, col) = if c == '\n' {
                    // Note that position of current row can be derived from new cursor
                    // position.
                    let (row_pos, _) = self.find_up(cur_pos + 1, 1);
                    (row_pos, (cur_pos - row_pos) as u32)
                } else {
                    // Prior row must be at least as long as window width because
                    // character before cursor is not \n, which means prior row must
                    // have soft wrapped.
                    (cur_pos + 1 - self.size.cols as usize, self.size.cols - 1)
                };

                // Changes to canvas only occur when cursor is already at top row of
                // window, otherwise just calculate new row number.
                let row = if self.cursor.row > 0 {
                    self.cursor.row - 1
                } else {
                    self.canvas_mut().scroll_down(0, 1);
                    self.render_rows(0, 1, row_pos);
                    self.canvas_mut().draw();
                    0
                };
                self.row_pos = row_pos;
                (row, col)
            };

            self.cur_pos = cur_pos;
            self.set_cursor(row, col);
        }
    }

    pub fn move_right(&mut self) {
        // Tries to move buffer position right by 1 character, though it may already be
        // at end of buffer.
        let right = self.buffer().forward(self.cur_pos).index().next();

        if let Some((cur_pos, c)) = right {
            // Calculate new column number based on adjacent character and current
            // location of cursor.
            let col = if c == '\n' {
                0
            } else {
                if self.cursor.col < self.size.cols - 1 {
                    self.cursor.col + 1
                } else {
                    0
                }
            };

            // New column number at left edge of window implies that cursor wraps to
            // next row, which may require changes to canvas.
            let row = if col == 0 {
                self.row_pos = cur_pos + 1;
                if self.cursor.row < self.size.rows - 1 {
                    self.cursor.row + 1
                } else {
                    self.canvas_mut().scroll_up(self.size.rows, 1);
                    self.render_rows(self.cursor.row, 1, self.row_pos);
                    self.canvas_mut().draw();
                    self.cursor.row
                }
            } else {
                self.cursor.row
            };

            self.cur_pos = cur_pos + 1;
            self.set_cursor(row, col);
        }
    }

    pub fn move_page_up(&mut self) {
        // Tries to move cursor up by number of rows equal to size of window, though top of
        // buffer could be reached first.
        let (row_pos, rows) = self.find_up(self.row_pos, self.size.rows);
        if rows > 0 {
            // Tries to maintain current location of cursor on display by moving up
            // additional rows, though again, top of buffer could be reached first.
            let (cur_pos, col) = self.find_col(row_pos, self.cursor.col);
            let (top_pos, row) = self.find_up(row_pos, self.cursor.row);

            // Since entire display likely changed in most cases, perform full rendering of
            // canvas before drawing.
            self.render_rows_from(0, top_pos);
            self.canvas_mut().draw();

            self.cur_pos = cur_pos;
            self.row_pos = row_pos;
            self.set_cursor(row, col);
        }
    }

    pub fn move_page_down(&mut self) {
        // Tries to move cursor down by number of rows equal to size of window, though
        // bottom of buffer could be reached first.
        let (row_pos, rows) = self.find_down(self.row_pos, self.size.rows);
        if rows > 0 {
            // Tries to maintain current location of cursor on display by moving down
            // additional rows, though again, bottom of buffer could be reached first.
            let (cur_pos, col) = self.find_col(row_pos, self.cursor.col);
            let (top_pos, row) = self.find_up(row_pos, self.cursor.row);

            // Since entire display likely changed in most cases, perform full rendering of
            // canvas before drawing.
            self.render_rows_from(0, top_pos);
            self.canvas_mut().draw();

            self.cur_pos = cur_pos;
            self.row_pos = row_pos;
            self.set_cursor(row, col);
        }
    }

    /// Moves the cursor to the beginning of the current row.
    pub fn move_beg(&mut self) {
        // Adjust cursor position and column if cursor not already at beginning of row.
        if self.cursor.col > 0 {
            self.cur_pos = self.row_pos;
            self.set_cursor_col(0);
        }
    }

    /// Moves the cursor to the end of the current row.
    pub fn move_end(&mut self) {
        // Try moving forward in buffer to find end of line, though stop if distance
        // between current column and right edge of window reached.
        let n = (self.size.cols - self.cursor.col - 1) as usize;
        let cur_pos = self.buffer().find_end_line_or(self.cur_pos, n);

        // Adjust cursor position and column if cursor not already at end of row.
        if cur_pos > self.cur_pos {
            self.cur_pos = cur_pos;
            self.set_cursor_col((self.cur_pos - self.row_pos) as u32);
        }
    }

    /// Moves the cursor to the top of the buffer.
    pub fn move_top(&mut self) {
        // Just render all rows since this is likely to happen in most cases, though
        // simple optimization is to ignore when cursor is already at top of buffer.
        if self.row_pos > 0 {
            self.row_pos = 0;
            self.render_rows_from(0, self.row_pos);
            self.canvas_mut().draw();
        }

        self.cur_pos = 0;
        self.set_cursor(0, 0);
    }

    /// Moves the cursor to the bottom of the buffer.
    pub fn move_bottom(&mut self) {
        // Determine column number at end of buffer.
        let cur_pos = self.buffer().size();
        self.cur_pos = cur_pos;
        let col = self.column_of(self.cur_pos);

        // Try to position cursor on last row of window, though this is only advisory
        // since buffer contents could be smaller than window.
        self.row_pos = self.cur_pos - col as usize;
        let (top_pos, row) = self.find_up(self.row_pos, self.size.rows - 1);
        self.render_rows_from(0, top_pos);
        self.canvas_mut().draw();
        self.set_cursor(row, col);
    }

    /// Scrolls the contents of the window up while preserving the cursor position, which
    /// means the cursor moves up as the contents scroll.
    ///
    /// If this operation would result in the cursor moving beyond the top row, then it
    /// is moved to the next row, essentially staying on the top row.
    pub fn scroll_up(&mut self) {
        // Try to find position of row following bottom row.
        let try_rows = self.size.rows - self.cursor.row;
        let (row_pos, rows) = self.find_down(self.row_pos, try_rows);

        // Only need to scroll if following row exiats.
        if rows == try_rows {
            self.canvas_mut().scroll_up(self.size.rows, 1);
            self.render_rows(self.size.rows - 1, 1, row_pos);
            self.canvas_mut().draw();

            let (row, col) = if self.cursor.row > 0 {
                // Indicates that cursor is not yet on top row.
                (self.cursor.row - 1, self.cursor.col)
            } else {
                // Indicates that cursor is already on top row, so new row position and
                // column number need to be calculated.
                let (row_pos, _) = self.find_down(self.row_pos, 1);
                let (cur_pos, col) = self.find_col(row_pos, self.cursor.col);
                self.cur_pos = cur_pos;
                self.row_pos = row_pos;
                (0, col)
            };
            self.set_cursor(row, col);
        }
    }

    /// Scrolls the contents of the window down while preserving the cursor position, which
    /// means the cursor moves down as the contents scroll.
    ///
    /// If this operation would result in the cursor moving beyond the bottom row, then it
    /// is moved to the previous row, essentiall staying on the bottow row.
    pub fn scroll_down(&mut self) {
        // Try to find position of row preceding top row.
        let try_rows = self.cursor.row + 1;
        let (row_pos, rows) = self.find_up(self.row_pos, try_rows);

        // Only need to scroll if preceding row exists.
        if rows == try_rows {
            self.canvas_mut().scroll_down(0, 1);
            self.render_rows(0, 1, row_pos);
            self.canvas_mut().draw();

            let (row, col) = if self.cursor.row < self.size.rows - 1 {
                // Indicates that cursor is not yet on bottom row.
                (self.cursor.row + 1, self.cursor.col)
            } else {
                // Indicates that cursor is already on bottom row, so new row position and
                // column number need to be calculated.
                let (row_pos, _) = self.find_up(self.row_pos, 1);
                let (cur_pos, col) = self.find_col(row_pos, self.cursor.col);
                self.cur_pos = cur_pos;
                self.row_pos = row_pos;
                (self.cursor.row, col)
            };
            self.set_cursor(row, col);
        }
    }

    /// Sets the cursor to (`row`, `col`) and updates the window.
    fn set_cursor(&mut self, row: u32, col: u32) {
        self.cursor = Point::new(row, col);
        self.canvas_mut().set_cursor(self.cursor);
    }

    /// Sets the cursor column to `col`, retaining the current row, and updates the
    /// window.
    fn set_cursor_col(&mut self, col: u32) {
        self.set_cursor(self.cursor.row, col);
    }

    /// Writes the contents of the buffer to the window, where `row` is the beginning row,
    /// `rows` is the number of rows, and `row_pos` is the buffer position of `row`.
    fn render_rows(&mut self, row: u32, rows: u32, row_pos: usize) {
        debug_assert!(row < self.size.rows);
        debug_assert!(row + rows <= self.size.rows);

        // Objective of this loop is to write specified range of rows to window.
        let end_row = row + rows;
        let mut row = row;
        let mut col = 0;

        for c in self.buffer.borrow().forward(row_pos) {
            if c == '\n' {
                self.canvas_mut().clear_row_from(row, col);
                col = self.size.cols;
            } else {
                let color = self.canvas().color();
                self.canvas_mut().set_cell(row, col, Cell::new(c, color));
                col += 1;
            }
            if col == self.size.cols {
                row += 1;
                col = 0;
            }
            if row == end_row {
                break;
            }
        }

        // Blanks out any remaining cells if end of buffer is reached for all rows not yet
        // processed.
        if row < end_row {
            self.canvas_mut().clear_row_from(row, col);
            self.canvas_mut().clear_rows(row + 1, end_row);
        }
    }

    /// Writes the contents of the buffer to the window, where `row` is the beginning row
    /// and `row_pos` is the buffer position of `row`.
    fn render_rows_from(&mut self, row: u32, row_pos: usize) {
        self.render_rows(row, self.size.rows - row, row_pos);
    }

    /// Finds the buffer position of `rows` preceding `row_pos`.
    ///
    /// Returns a tuple containing the (row position, rows), where the number of rows may
    /// be less than `rows` if the operation reaches the beginning of the buffer.
    fn find_up(&self, row_pos: usize, rows: u32) -> (usize, u32) {
        let mut row = 0;
        let mut row_pos = row_pos;

        while row < rows {
            if let Some(pos) = self.prev_row(row_pos) {
                row_pos = pos;
                row += 1;
            } else {
                break;
            }
        }
        (row_pos, row)
    }

    /// Finds the buffer position of `rows` following `row_pos`.
    ///
    /// Returns a tuple containing the (row position, rows), where the number of rows may
    /// be less than `rows` if the operation reaches the end of buffer.
    fn find_down(&self, row_pos: usize, rows: u32) -> (usize, u32) {
        let mut row = 0;
        let mut row_pos = row_pos;

        while row < rows {
            if let Some(pos) = self.next_row(row_pos) {
                row_pos = pos;
                row += 1;
            } else {
                break;
            }
        }
        (row_pos, row)
    }

    /// Returns the buffer position of the beginning of the previous row relative to
    /// `row_pos`, which is assumed to be the beginning position of the current row.
    ///
    /// Returns `None` if the current row is already at the top of the buffer.
    fn prev_row(&self, row_pos: usize) -> Option<usize> {
        if row_pos > 0 {
            let offset = match self.buffer().get_char(row_pos - 1) {
                // Indicates that current row position is also beginning of line, so
                // determine offset to prior row by finding beginning of prior line.
                Some('\n') => {
                    let pos = self.buffer().find_beg_line(row_pos - 1);
                    let offset = (row_pos - pos) % self.size.cols as usize;
                    if offset > 0 {
                        offset
                    } else {
                        self.size.cols as usize
                    }
                }
                // Indicates that current row position is not beginning of line, which
                // means it wrapped, making offset to prior row trivially equal to width
                // of window.
                _ => self.size.cols as usize,
            };
            Some(row_pos - offset)
        } else {
            None
        }
    }

    /// Returns the buffer position of the beginning of the next row relative to
    /// `row_pos`, which is assumed to be the beginning position of the current row.
    ///
    /// Retuns `None` if the current row is alredy at the bottom of the buffer.
    fn next_row(&self, row_pos: usize) -> Option<usize> {
        // Scans forward until \n encountered or number of characters not to exceed
        // width of window.
        self.buffer()
            .find_next_line_or(row_pos, self.size.cols as usize)
    }

    fn find_row_pos(&self, pos: usize) -> usize {
        pos - ((pos - self.buffer().find_beg_line(pos)) % self.size.cols as usize)
    }

    /// Finds the buffer position of `col` relative to `row_pos`, which is assumed to be
    /// the position corresponding to column `0`.
    ///
    /// Returns a tuple containing the _actual_ (column position, column number), which may
    /// not correspond to `col`. Since `col` may extend beyond the end of line, the actual
    /// column is bounded as such.
    fn find_col(&self, row_pos: usize, col: u32) -> (usize, u32) {
        // Scans forward until \n encountered or number of characters processed reaches
        // specified column.
        let col_pos = self.buffer().find_end_line_or(row_pos, col as usize);
        (col_pos, (col_pos - row_pos) as u32)
    }

    /// Returns the column number corresponding to `pos`.
    fn column_of(&self, pos: usize) -> u32 {
        // Column number is derived by calculating distance between given position
        // and beginning of line, though bounded by width of window.
        (pos - self.buffer().find_beg_line(pos)) as u32 % self.size.cols
    }

    /// Returns the number of rows that wrap beyond the current row designated by
    /// `row_pos`.
    fn wrapped_rows(&self, row_pos: usize) -> u32 {
        (self.buffer().find_end_line(row_pos) - row_pos) as u32 / self.size.cols
    }

    /// Finds the cursor and corresponding row position at the given cursor `pos`, returning
    /// the tuple (row, col, row_pos).
    ///
    /// Note that the resulting _row_ is an `Option<u32>` because the calculated row
    /// may extend beyond the visible portion of the window. Specifically, if the
    /// resulting row would be outside the range [`0`, `self.window.rows()`), then the
    /// value is `None`.
    ///
    /// This computation is relative to the current cursor position, which is assumed to
    /// be in a correct state prior to calling this function.
    fn find_cursor(&self, pos: usize) -> (Option<u32>, u32, usize) {
        if pos > self.cur_pos {
            let (row, col, row_pos) = self
                .buffer()
                .forward(self.cur_pos)
                .index()
                .take(pos - self.cur_pos)
                .fold(
                    (self.cursor.row, self.cursor.col, self.row_pos),
                    |(row, col, row_pos), (pos, c)| {
                        if c == '\n' || col == self.size.cols - 1 {
                            // Move to next row when \n is encountered or cursor at right edge of
                            // window, nothing that next row position always follows the current
                            // character.
                            (row + 1, 0, pos + 1)
                        } else {
                            (row, col + 1, row_pos)
                        }
                    },
                );
            let maybe_row = if row < self.size.rows {
                Some(row)
            } else {
                None
            };
            (maybe_row, col, row_pos)
        } else if pos < self.cur_pos {
            let (rows, _) = self
                .buffer()
                .backward(self.cur_pos)
                .index()
                .take(self.cur_pos - pos)
                .fold((0, self.cursor.col), |(rows, col), (_, c)| {
                    if c == '\n' || col == 0 {
                        (rows + 1, self.size.cols - 1)
                    } else {
                        (rows, col - 1)
                    }
                });
            let row_pos = self.find_row_pos(pos);
            let maybe_row = if rows <= self.cursor.row {
                Some(self.cursor.row - rows)
            } else {
                None
            };
            (maybe_row, (pos - row_pos) as u32, row_pos)
        } else {
            (Some(self.cursor.row), self.cursor.col, self.row_pos)
        }
    }
}
