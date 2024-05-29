//! Document.

use crate::buffer::Buffer;
use crate::canvas::{Cell, Point};
use crate::window::Window;
use std::cmp;

pub struct Document {
    buffer: Buffer,
    window: Window,
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

pub struct Cursor {
    pos: usize,
    point: Point,
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

impl Document {
    pub fn new(buffer: Buffer, window: Window) -> Document {
        let mut doc = Document {
            buffer,
            window,
            cursor: Point::new(0, 0),
        };
        doc.align_cursor(Focus::Auto);
        doc
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    // probably don't want to do this directly from document
    pub fn window(&mut self) -> &mut Window {
        &mut self.window
    }

    pub fn align_cursor(&mut self, focus: Focus) {
        // Determine ideal row where cursor would like to be focused, though this should
        // be considered a hint.
        let row = match focus {
            Focus::Auto => self.window.rows() / 2,
            Focus::Row(row) => cmp::min(row, self.window.rows() - 1),
        };

        // Tries to position cursor on target row, but no guarantee depending on proximity
        // of row to top of buffer.
        self.cursor.col = self.detect_col(self.buffer.get_pos());
        let (origin_pos, rows) = self.find_up(row);

        // Renders the entire back canvas before drawing.
        self.render_rows(0, self.window.rows(), origin_pos);
        self.window.draw();

        // Set cursor position on display.
        self.cursor.row = rows;
        self.window.set_cursor(self.cursor);
    }

    pub fn move_cursor(&mut self, dir: Direction) -> usize {
        let cursor = match dir {
            Direction::Up => self.move_up(),
            Direction::Down => self.move_down(),
            Direction::Left => self.move_left(),
            Direction::Right => self.move_right(),
            Direction::PageUp => self.move_page_up(),
            Direction::PageDown => self.move_page_down(),
        };

        if let Some(c) = cursor {
            self.cursor = c;
            self.window.set_cursor(self.cursor);
        }

        self.buffer.get_pos()
    }

    fn move_up(&mut self) -> Option<Point> {
        // Tries to move cursor up by 1 row, though it may already be at top of buffer.
        let (row_pos, rows) = self.find_up(1);
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

            // Set cursor position in buffer.
            self.buffer.set_pos(cursor_pos);
            Some(Point::new(row, col))
        } else {
            // Already at top of buffer.
            None
        }
    }

    fn move_page_up(&mut self) -> Option<Point> {
        // Tries to move cursor up by number of rows equal to size of window, though top of
        // buffer could be reached first.
        let (row_pos, rows) = self.find_up(self.window.rows());
        if rows > 0 {
            // Tries to maintain current location of cursor on display by moving up
            // additional rows, though again, top of buffer could be reached first.
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);
            let (origin_pos, row) = self.find_up_from(row_pos, self.cursor.row);

            // Since entire display likely changed in most cases, perform full rendering of
            // canvas before drawing.
            self.render_rows(0, self.window.rows(), origin_pos);
            self.window.draw();

            // Set cursor position in buffer.
            self.buffer.set_pos(cursor_pos);
            Some(Point::new(row, col))
        } else {
            // Already at top of buffer.
            None
        }
    }

    fn move_down(&mut self) -> Option<Point> {
        // Tries to move cursor down by 1 row, though it may already be at end of buffer.
        let (row_pos, rows) = self.find_down(1);
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

            // Set cursor position in buffer.
            self.buffer.set_pos(cursor_pos);
            Some(Point::new(row, col))
        } else {
            // Already at end of buffer.
            None
        }
    }

    fn move_page_down(&mut self) -> Option<Point> {
        // Tries to move cursor down by number of rows equal to size of window, though
        // bottom of buffer could be reached first.
        let (row_pos, rows) = self.find_down(self.window.rows());
        if rows > 0 {
            // Tries to maintain current location of cursor on display by moving down
            // additional rows, though again, bottom of buffer could be reached first.
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);
            let (origin_pos, row) = self.find_up_from(row_pos, self.cursor.row);

            // Since entire display likely changed in most cases, perform full rendering of
            // canvas before drawing.
            self.render_rows(0, self.window.rows(), origin_pos);
            self.window.draw();

            // Set cursor position in buffer.
            self.buffer.set_pos(cursor_pos);
            Some(Point::new(row, col))
        } else {
            None
        }
    }

    fn move_left(&mut self) -> Option<Point> {
        // Tries to move buffer position left by 1 character, though it may already be
        // at beginning of buffer.
        let left = self.buffer.backward().index().next();

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
                    let (row_pos, _) = self.find_up_from(cursor_pos + 1, 1);
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
                (row, col)
            };

            // Set cursor position in buffer.
            self.buffer.set_pos(cursor_pos);
            Some(Point::new(row, col))
        } else {
            // Already at beginning of buffer.
            None
        }
    }

    fn move_right(&mut self) -> Option<Point> {
        // Tries to move buffer position right by 1 character, though it may already be
        // at end of buffer.
        let right = self.buffer.forward().index().next();

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
                if self.cursor.row < self.window.rows() - 1 {
                    self.cursor.row + 1
                } else {
                    self.window.scroll_up(self.window.rows(), 1);
                    self.render_rows(self.cursor.row, 1, cursor_pos + 1);
                    self.window.draw();
                    self.cursor.row
                }
            } else {
                self.cursor.row
            };

            // Set cursor position in buffer.
            self.buffer.set_pos(cursor_pos + 1);
            Some(Point::new(row, col))
        } else {
            // Already at end of buffer.
            None
        }
    }

    fn render_rows(&mut self, row: u32, rows: u32, row_pos: usize) {
        assert!(row < self.window.rows());
        assert!(row + rows <= self.window.rows());

        // Objective of this loop is to write specified range of rows to window.
        let end_row = row + rows;
        let mut row = row;
        let mut col = 0;

        for c in self.buffer.forward_from(row_pos) {
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

    fn find_up(&self, rows: u32) -> (usize, u32) {
        self.find_up_from(self.find_cur_row(), rows)
    }

    fn find_up_from(&self, row_pos: usize, rows: u32) -> (usize, u32) {
        let mut row = 0;
        let mut row_pos = row_pos;

        while row < rows {
            if let Some(pos) = self.find_prev_row(row_pos) {
                row_pos = pos;
                row += 1;
            } else {
                break;
            }
        }
        (row_pos, row)
    }

    fn find_down(&self, rows: u32) -> (usize, u32) {
        self.find_down_from(self.find_cur_row(), rows)
    }

    fn find_down_from(&self, row_pos: usize, rows: u32) -> (usize, u32) {
        let mut row = 0;
        let mut row_pos = row_pos;

        while row < rows {
            if let Some(pos) = self.find_next_row(row_pos) {
                row_pos = pos;
                row += 1;
            } else {
                break;
            }
        }
        (row_pos, row)
    }

    // returns buffer position of cursor row
    fn find_cur_row(&self) -> usize {
        self.buffer.get_pos() - self.cursor.col as usize
    }

    // returns buffer position of previous row, where row_pos is assumed to be beginning of
    // current row.
    //
    // returns None if current row is already at top.
    fn find_prev_row(&self, row_pos: usize) -> Option<usize> {
        if row_pos > 0 {
            let pos = self.buffer.find_bol(row_pos - 1);

            // // Scans backward until \n encountered, skipping first character since it
            // // may be \n if prior row did not wrap. Result identifies beginning of prior
            // // line (not row), which could be larger than width of window.
            // let result = self
            //     .buffer
            //     .backward_from(row_pos)
            //     .index()
            //     .skip(1)
            //     .find(|&(_, c)| c == '\n');

            // // Distance between current row position and prior line position could be
            // // larger than width of window.
            // let pos = match result {
            //     Some((pos, _)) => pos + 1,
            //     None => 0,
            // };

            let delta = (row_pos - pos) % self.window.cols() as usize;
            Some(
                row_pos
                    - if delta > 0 {
                        delta
                    } else {
                        self.window.cols() as usize
                    },
            )
        } else {
            // Already at top row of buffer.
            None
        }
    }

    // returns buffer position of next row, where row_pos is assumed to be beginning of
    // current row.
    //
    // returns None if current row is already at bottom.
    fn find_next_row(&self, row_pos: usize) -> Option<usize> {
        // Scans forward until \n encountered or number of characters not to exceed
        // width of window.
        self.buffer
            .find_next_or(row_pos, self.window.cols() as usize)
    }

    // finds buffer position of specified column relative to row_pos, where row_pos is assumed
    // to be current row.
    //
    // returns tuple of (buffer position, actual column), where actual column may be less
    // than specified column.
    fn find_col(&self, row_pos: usize, col: u32) -> (usize, u32) {
        // Scans forward until \n encountered or number of characters processed reaches
        // specified column.
        let col_pos = self.buffer.find_eol_or(row_pos, col as usize);
        (col_pos, (col_pos - row_pos) as u32)
    }

    // returns column number corresponding to given buffer position, which is
    // always <= self.cols
    fn detect_col(&self, pos: usize) -> u32 {
        // Scan backwards to find first \n or beginning of buffer, whichever comes first,
        // denoting beginning of line (not row).
        let line_pos = self.buffer.find_bol(pos);

        // Column numnber can be easily derived based on width of window.
        (pos - line_pos) as u32 % self.window.cols()
    }
}
