//! Document.

use crate::buffer::Buffer;
use crate::canvas::{Cell, Point};
use crate::window::Window;
use std::cmp;

pub struct Document {
    buffer: Buffer,
    window: Window,
    cursor_pos: usize,
    row_pos: usize,
    cursor: Point,
}

pub enum Focus {
    Auto,
    Row(u32),
}

pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
}

// thinking about operations on the window. there appear to be two kinds of operations
// - movements relative to the cursor, such move left or page down.
// - movements relative to an edit, such as inserting a character or cuttting a region.
//
// movements relative to cursor:
// - the editor reads a movement key, such as move 1 line up.
// - instruct the window to move up 1 line. the window has the ability to navigate the buffer
//   to determine how to repaint the canvas.
// - the window should return the new buffer position to the editor, which allows it to
//   update its own state.
// - in general, tell the window how it needs to move within the buffer, and it will return
//   the new buffer positon. a side effect of any move is that the window may repaint the
//   canvas to ensure the new buffer position is visible to the user. such repainting is
//   a concern of the window, not the editor.
//
// movements relative to an edit:
// - the editor reads a key that results in a change to the buffer. the editor needs to
//   orchestrate this change.
// - instruct the window to focus the cursor on the new buffer position that resulted
//   from the edit. an example is inserting a character, which moves the cursor to the
//   right or may cause it to wrap to a line that is not yet visible.
// - the window is solely responsible for dispalying the buffer to the user, so the
//   editor is not able to tell it precisely how to behave. example: suppose the cursor
//   is at the bottom-right-most cell of the buffer when the user inserts a character.
//   depending on the scrolling behavior of the window, it will scroll down one or
//   more lines. the editor should not be burdened with this responsibility.
// - the editor could give a hint to the window by telling it which area of the buffer
//   was changed. if a character is inserted at buffer pos n, then tell the window as
//   such: window.insert(n, 1). if a block of size m is inserted at position n, then
//   inform as such: window.insert(n, m). if a character is deleted at position n, then
//   call: window.remove(n, 1). if a block of size m is deleted at position n, then
//   call: window.remove(n, m). the hint will allow the window to optimize its work.
//
// struct Cursor
// - pos: usize    position in buffer
// - point: Point  (x,y) in window
//
// note that pos does not necessarily means the gap pos in the buffer. since many
// buffer operations are movement, we can avoid continuously moving the gap around
// until there is a mutating operation.
//
// essentially, a mutating operation needs to ensure:
//   buffer.set_pos(cursor.pos);
//
// given:
// - buffer pos
// - preferred placement of cursor
// determine:
// - cursor (row, col) corresponding to buffer pos

impl Document {
    pub fn new(buffer: Buffer, window: Window) -> Document {
        let cursor_pos = buffer.get_pos();
        let mut doc = Document {
            buffer,
            window,
            cursor_pos,
            row_pos: 0,
            cursor: Point::ORIGIN,
        };
        doc.align_cursor(Focus::Auto);
        doc
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn render(&mut self) {
        let (top_pos, _) = self.find_up(self.row_pos, self.cursor.row);
        self.render_rows(0, self.window.rows(), top_pos);
        self.window.redraw();
        self.window.set_cursor(self.cursor);
    }

    pub fn align_cursor(&mut self, focus: Focus) {
        // Determine ideal row where cursor would like to be focused, though this should
        // be considered a hint.
        let row = match focus {
            Focus::Auto => self.window.rows() / 2,
            Focus::Row(row) => cmp::min(row, self.window.rows() - 1),
        };

        // Column number can be derived by calculating distance between cursor position
        // and beginning of line, though bounded by width of window.
        let col = (self.cursor_pos - self.buffer.find_beg_line(self.cursor_pos)) as u32
            % self.window.cols();

        // Tries to position cursor on target row, but no guarantee depending on proximity
        // of row to top of buffer.
        self.row_pos = self.cursor_pos - col as usize;
        let (top_pos, row) = self.find_up(self.row_pos, row);
        self.render_rows(0, self.window.rows(), top_pos);
        self.window.draw();
        self.cursor = Point::new(row, col);
        self.window.set_cursor(self.cursor);
    }

    pub fn move_cursor(&mut self, dir: Direction) -> usize {
        match dir {
            Direction::Up => self.move_up(),
            Direction::Down => self.move_down(),
            Direction::Left => self.move_left(),
            Direction::Right => self.move_right(),
            Direction::PageUp => self.move_page_up(),
            Direction::PageDown => self.move_page_down(),
        };
        self.window.set_cursor(self.cursor);
        self.buffer.get_pos()
    }

    pub fn move_beg(&mut self) {
        if self.cursor.col > 0 {
            self.cursor_pos = self.row_pos;
            self.cursor = Point::new(self.cursor.row, 0);
            self.window.set_cursor(self.cursor);
        }
    }

    pub fn move_end(&mut self) {
        // determine max number of characters to move right based on cursor pos.
        // this will never underflow because width of window must always be > 0.
        let n = (self.window.cols() - self.cursor.col - 1) as usize;

        let cursor_pos = self.buffer.find_end_line_or(self.cursor_pos, n);
        if cursor_pos > self.cursor_pos {
            // moved at least one character
            self.cursor_pos = cursor_pos;
            self.cursor = Point::new(self.cursor.row, (self.cursor_pos - self.row_pos) as u32);
            self.window.set_cursor(self.cursor);
        }
    }

    pub fn move_top(&mut self) {
        // possible to optimize rendering if already at top of file, even though
        // cursor may not be at top.
        self.row_pos = 0;
        self.render_rows(0, self.window.rows(), self.row_pos);
        self.window.draw();

        self.cursor_pos = 0;
        self.cursor = Point::new(0, 0);
        self.window.set_cursor(self.cursor);
    }

    pub fn move_bottom(&mut self) {
        self.cursor_pos = self.buffer.size();

        let col = (self.cursor_pos - self.buffer.find_beg_line(self.cursor_pos)) as u32
            % self.window.cols();

        self.row_pos = self.cursor_pos - col as usize;
        let (top_pos, row) = self.find_up(self.row_pos, self.window.rows() - 1);
        self.render_rows(0, self.window.rows(), top_pos);
        self.window.draw();
        self.cursor = Point::new(row, col);
        self.window.set_cursor(self.cursor);
    }

    // scroll window up without moving cursor position
    // cursor moves up with text, but stays focused on same position
    pub fn scroll_up(&mut self) {
        // this will find the row pos following the last row on display
        let try_rows = self.window.rows() - self.cursor.row;
        let (row_pos, rows) = self.find_down(self.row_pos, try_rows);
        if rows == try_rows {
            // this means we are not yet at bottom of buffer
            self.window.scroll_up(self.window.rows(), 1);
            self.render_rows(self.window.rows() - 1, 1, row_pos);
            self.window.draw();

            let (row, col) = if self.cursor.row > 0 {
                // cursor not on top row, row pos remains unchanged
                (self.cursor.row - 1, self.cursor.col)
            } else {
                // cursor on top row, need to find new row pos and col
                // because the display scrolled, we know that next row pos will be found
                let (row_pos, _) = self.find_down(self.row_pos, 1);
                let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);
                self.cursor_pos = cursor_pos;
                self.row_pos = row_pos;
                (0, col)
            };
            self.cursor = Point::new(row, col);
            self.window.set_cursor(self.cursor);
        }
    }

    // scroll window down without moving cursor position
    // cursor moves down with text, but stays focused on the same position
    pub fn scroll_down(&mut self) {
        let try_rows = self.cursor.row + 1;
        let (row_pos, rows) = self.find_up(self.row_pos, try_rows);
        if rows == try_rows {
            // this means we are not yet at the top of buffer
            self.window.scroll_down(0, 1);
            self.render_rows(0, 1, row_pos);
            self.window.draw();

            let (row, col) = if self.cursor.row < self.window.rows() - 1 {
                // cursor not on bottom row, row pos remains unchanged
                (self.cursor.row + 1, self.cursor.col)
            } else {
                // cursor on bottom row, need to find new row pos and col
                // because the display scrolled, we know that prev row pos will be found
                let (row_pos, _) = self.find_up(self.row_pos, 1);
                let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);
                self.cursor_pos = cursor_pos;
                self.row_pos = row_pos;
                (self.cursor.row, col)
            };
            self.cursor = Point::new(row, col);
            self.window.set_cursor(self.cursor);
        }
    }

    fn move_up(&mut self) {
        // Tries to move cursor up by 1 row, though it may already be at top of buffer.
        let (row_pos, rows) = self.find_up(self.row_pos, 1);
        if rows > 0 {
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);

            // Changes to canvas only occur when cursor is already at top row of window,
            // otherwise just change position of cursor on display.
            let row = if self.cursor.row > 0 {
                self.cursor.row - 1
            } else {
                self.window.scroll_down(0, 1);
                self.render_rows(0, 1, row_pos);
                self.window.draw();
                0
            };

            self.cursor_pos = cursor_pos;
            self.row_pos = row_pos;
            self.cursor = Point::new(row, col);
        }
    }

    fn move_page_up(&mut self) {
        // Tries to move cursor up by number of rows equal to size of window, though top of
        // buffer could be reached first.
        let (row_pos, rows) = self.find_up(self.row_pos, self.window.rows());
        if rows > 0 {
            // Tries to maintain current location of cursor on display by moving up
            // additional rows, though again, top of buffer could be reached first.
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);
            let (top_pos, row) = self.find_up(row_pos, self.cursor.row);

            // Since entire display likely changed in most cases, perform full rendering of
            // canvas before drawing.
            self.render_rows(0, self.window.rows(), top_pos);
            self.window.draw();

            self.cursor_pos = cursor_pos;
            self.row_pos = row_pos;
            self.cursor = Point::new(row, col);
        }
    }

    fn move_down(&mut self) {
        // Tries to move cursor down by 1 row, though it may already be at end of buffer.
        let (row_pos, rows) = self.find_down(self.row_pos, 1);
        if rows > 0 {
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);

            // Changes to canvas only occur when cursor is already at bottow row of
            // window, otherwise just change position of cursor on display.
            let row = if self.cursor.row < self.window.rows() - 1 {
                self.cursor.row + 1
            } else {
                self.window.scroll_up(self.window.rows(), 1);
                self.render_rows(self.cursor.row, 1, row_pos);
                self.window.draw();
                self.cursor.row
            };

            self.cursor_pos = cursor_pos;
            self.row_pos = row_pos;
            self.cursor = Point::new(row, col);
        }
    }

    fn move_page_down(&mut self) {
        // Tries to move cursor down by number of rows equal to size of window, though
        // bottom of buffer could be reached first.
        let (row_pos, rows) = self.find_down(self.row_pos, self.window.rows());
        if rows > 0 {
            // Tries to maintain current location of cursor on display by moving down
            // additional rows, though again, bottom of buffer could be reached first.
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);
            let (top_pos, row) = self.find_up(row_pos, self.cursor.row);

            // Since entire display likely changed in most cases, perform full rendering of
            // canvas before drawing.
            self.render_rows(0, self.window.rows(), top_pos);
            self.window.draw();

            self.cursor_pos = cursor_pos;
            self.row_pos = row_pos;
            self.cursor = Point::new(row, col);
        }
    }

    fn move_left(&mut self) {
        // Tries to move buffer position left by 1 character, though it may already be
        // at beginning of buffer.
        let left = self.buffer.backward(self.cursor_pos).index().next();

        if let Some((cursor_pos, c)) = left {
            let (row, col) = if self.cursor.col > 0 {
                // Cursor not at left edge of window, so just a simple cursor move.
                (self.cursor.row, self.cursor.col - 1)
            } else {
                // Cursor at left edge of window, so determine position of prior row
                // and column number.
                let (row_pos, col) = if c == '\n' {
                    // Note that position of current row can be derived from new cursor
                    // position.
                    let (row_pos, _) = self.find_up(cursor_pos + 1, 1);
                    (row_pos, (cursor_pos - row_pos) as u32)
                } else {
                    // Prior row must be at least as long as window width because
                    // character before cursor is not \n, which means prior row must
                    // have soft wrapped.
                    (
                        cursor_pos + 1 - self.window.cols() as usize,
                        self.window.cols() - 1,
                    )
                };

                // Changes to canvas only occur when cursor is already at top row of
                // window, otherwise just calculate new row number.
                let row = if self.cursor.row > 0 {
                    self.cursor.row - 1
                } else {
                    self.window.scroll_down(0, 1);
                    self.render_rows(0, 1, row_pos);
                    self.window.draw();
                    0
                };
                self.row_pos = row_pos;
                (row, col)
            };

            self.cursor_pos = cursor_pos;
            self.cursor = Point::new(row, col);
        }
    }

    fn move_right(&mut self) {
        // Tries to move buffer position right by 1 character, though it may already be
        // at end of buffer.
        let right = self.buffer.forward(self.cursor_pos).index().next();

        if let Some((cursor_pos, c)) = right {
            // Calculate new column number based on adjacent character and current
            // location of cursor.
            let col = if c == '\n' {
                0
            } else {
                if self.cursor.col < self.window.cols() - 1 {
                    self.cursor.col + 1
                } else {
                    0
                }
            };

            // New column number at left edge of window implies that cursor wraps to
            // next row, which may require changes to canvas.
            let row = if col == 0 {
                self.row_pos = cursor_pos + 1;
                if self.cursor.row < self.window.rows() - 1 {
                    self.cursor.row + 1
                } else {
                    self.window.scroll_up(self.window.rows(), 1);
                    self.render_rows(self.cursor.row, 1, self.row_pos);
                    self.window.draw();
                    self.cursor.row
                }
            } else {
                self.cursor.row
            };

            self.cursor_pos = cursor_pos + 1;
            self.cursor = Point::new(row, col);
        }
    }

    fn render_rows(&mut self, row: u32, rows: u32, row_pos: usize) {
        debug_assert!(row < self.window.rows());
        debug_assert!(row + rows <= self.window.rows());

        // Objective of this loop is to write specified range of rows to window.
        let end_row = row + rows;
        let mut row = row;
        let mut col = 0;

        for c in self.buffer.forward(row_pos) {
            if c == '\n' {
                self.window.clear_row_from(row, col);
                col = self.window.cols();
            } else {
                self.window
                    .set_cell(row, col, Cell::new(c, self.window.color()));
                col += 1;
            }
            if col == self.window.cols() {
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
            self.window.clear_row_from(row, col);
            self.window.clear_rows(row + 1, end_row);
        }
    }

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
            let offset = match self.buffer.get(row_pos - 1) {
                // Indicates that current row position is also beginning of line, so
                // determine offset to prior row by finding beginning of prior line.
                Some('\n') => {
                    let pos = self.buffer.find_beg_line(row_pos - 1);
                    let offset = (row_pos - pos) % self.window.cols() as usize;
                    if offset > 0 {
                        offset
                    } else {
                        self.window.cols() as usize
                    }
                }
                // Indicates that current row position is not beginning of line, which
                // means it wrapped, making offset to prior row trivially equal to width
                // of window.
                _ => self.window.cols() as usize,
            };
            Some(row_pos - offset)
        } else {
            None
        }
    }

    // returns buffer position of next row, where row_pos is assumed to be beginning of
    // current row.
    //
    // returns None if current row is already at bottom.
    fn next_row(&self, row_pos: usize) -> Option<usize> {
        // Scans forward until \n encountered or number of characters not to exceed
        // width of window.
        self.buffer
            .find_next_line_or(row_pos, self.window.cols() as usize)
    }

    // finds buffer position of specified column relative to row_pos, where row_pos is assumed
    // to be current row.
    //
    // returns tuple of (buffer position, actual column), where actual column may be less
    // than specified column.
    fn find_col(&self, row_pos: usize, col: u32) -> (usize, u32) {
        // Scans forward until \n encountered or number of characters processed reaches
        // specified column.
        let col_pos = self.buffer.find_end_line_or(row_pos, col as usize);
        (col_pos, (col_pos - row_pos) as u32)
    }
}
