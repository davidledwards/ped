//! Window management.

use crate::ansi;
use crate::buffer::{BackwardIndex, Buffer};
use crate::canvas::{Canvas, Cell, Point};
use crate::color::Color;
use crate::display::Display;
use std::cell::RefCell;
use std::cmp;
use std::rc::Rc;

pub struct Window {
    rows: u32,
    cols: u32,
    color: Color,
    cursor: Point,
    buffer: Rc<RefCell<Buffer>>,
    back: Canvas,
    front: Canvas,
    display: Display,
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

// consider using builder pattern due to number of variations in configuration
// some can be defaulted but could be overridden
// others are mandatory, so accept as input to construct the builder

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

impl Window {
    pub fn new(
        rows: u32,
        cols: u32,
        color: Color,
        origin: Point,
        buffer: Rc<RefCell<Buffer>>,
    ) -> Window {
        assert!(rows > 0);
        assert!(cols > 0);

        let mut win = Window {
            rows,
            cols,
            color,
            cursor: Point::new(0, 0),
            buffer,
            back: Canvas::new(rows, cols),
            front: Canvas::new(rows, cols),
            display: Display::new(rows, cols, origin),
        };

        win.align_cursor(Focus::Auto);
        win
    }

    pub fn align_cursor(&mut self, focus: Focus) {
        // Determine ideal row where cursor would like to be focused, though this should
        // be considered a hint.
        let row = match focus {
            Focus::Auto => self.rows / 2,
            Focus::Row(row) => cmp::min(row, self.rows - 1),
        };

        // Tries to position cursor on target row, but no guarantee depending on proximity
        // of row to top of buffer.
        self.cursor.col = self.detect_col();
        let (origin_pos, rows) = self.find_up(row);

        // Renders the entire back canvas before drawing.
        self.render_rows(0, self.rows, origin_pos);
        self.draw();

        // Set cursor position on display.
        self.cursor.row = rows;
        self.display.write_cursor(self.cursor);
        self.display.send();
    }

    pub fn move_cursor(&mut self, dir: Direction) -> usize {
        match dir {
            Direction::Up => self.move_up(),
            Direction::Down => self.move_down(),
            Direction::Left => self.move_left(),
            Direction::Right => self.move_right(),
            Direction::PageUp => self.move_page_up(),
            Direction::PageDown => self.move_page_down(),
        }
    }

    fn move_up(&mut self) -> usize {
        // Tries to move cursor up by 1 row, though it may already be at top of buffer.
        let (row_pos, rows) = self.find_up(1);
        if rows > 0 {
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);

            // Changes to canvas only occur when cursor is already at top row of window,
            // otherwise just change position of cursor on display.
            let row = if self.cursor.row > 0 {
                self.cursor.row - 1
            } else {
                self.back.shift_down(1);
                self.render_rows(0, 1, row_pos);
                self.draw();
                0
            };

            // Set cursor position on display.
            self.cursor = Point::new(row, col);
            self.display.write_cursor(self.cursor);
            self.display.send();

            // Set cursor position in buffer.
            self.buffer.borrow_mut().set_pos(cursor_pos)
        } else {
            // Already at top of buffer.
            self.buffer.borrow().get_pos()
        }
    }

    fn move_page_up(&mut self) -> usize {
        // Tries to move cursor up by number of rows equal to size of window, though top of
        // buffer could be reached first.
        let (row_pos, rows) = self.find_up(self.rows);
        if rows > 0 {
            // Tries to maintain current location of cursor on display by moving up
            // additional rows, though again, top of buffer could be reached first.
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);
            let (origin_pos, row) = self.find_up_from(row_pos, self.cursor.row);

            // Since entire display likely changed in most cases, perform full rendering of
            // canvas before drawing.
            self.render_rows(0, self.rows, origin_pos);
            self.draw();

            // Set cursor position on display.
            self.cursor = Point::new(row, col);
            self.display.write_cursor(self.cursor);
            self.display.send();

            // Set cursor position in buffer.
            self.buffer.borrow_mut().set_pos(cursor_pos)
        } else {
            // Already at top of buffer.
            self.buffer.borrow().get_pos()
        }
    }

    fn move_down(&mut self) -> usize {
        // Tries to move cursor down by 1 row, though it may already be at end of buffer.
        let (row_pos, rows) = self.find_down(1);
        if rows > 0 {
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);

            // Changes to canvas only occur when cursor is already at bottow row of
            // window, otherwise just change position of cursor on display.
            let row = if self.cursor.row < self.rows - 1 {
                self.cursor.row + 1
            } else {
                self.back.shift_up(1);
                self.render_rows(self.cursor.row, 1, row_pos);
                self.draw();
                self.cursor.row
            };

            // Set cursor position on display.
            self.cursor = Point::new(row, col);
            self.display.write_cursor(self.cursor);
            self.display.send();

            // Set cursor position in buffer.
            self.buffer.borrow_mut().set_pos(cursor_pos)
        } else {
            // Already at end of buffer.
            self.buffer.borrow().get_pos()
        }
    }

    fn move_page_down(&mut self) -> usize {
        // Tries to move cursor down by number of rows equal to size of window, though
        // bottom of buffer could be reached first.
        let (row_pos, rows) = self.find_down(self.rows);
        if rows > 0 {
            // Tries to maintain current location of cursor on display by moving down
            // additional rows, though again, bottom of buffer could be reached first.
            let (cursor_pos, col) = self.find_col(row_pos, self.cursor.col);
            let (origin_pos, row) = self.find_up_from(row_pos, self.cursor.row);

            // Since entire display likely changed in most cases, perform full rendering of
            // canvas before drawing.
            self.render_rows(0, self.rows, origin_pos);
            self.draw();

            // Set cursor position on display.
            self.cursor = Point::new(row, col);
            self.display.write_cursor(self.cursor);
            self.display.send();

            // Set cursor position in buffer.
            self.buffer.borrow_mut().set_pos(cursor_pos)
        } else {
            self.buffer.borrow().get_pos()
        }
    }

    fn move_left(&mut self) -> usize {
        // Tries to move buffer position left by 1 character, though it may already be
        // at beginning of buffer.
        let left = self.buffer.borrow().backward().index().next();

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
                    (cursor_pos + 1 - self.cols as usize, self.cols - 1)
                };

                // Changes to canvas only occur when cursor is already at top row of
                // window, otherwise just calculate new row number.
                let row = if self.cursor.row > 0 {
                    self.cursor.row - 1
                } else {
                    self.back.shift_down(1);
                    self.render_rows(0, 1, row_pos);
                    self.draw();
                    0
                };
                (row, col)
            };

            // Set cursor position on display.
            self.cursor = Point::new(row, col);
            self.display.write_cursor(self.cursor);
            self.display.send();

            // Set cursor position in buffer.
            self.buffer.borrow_mut().set_pos(cursor_pos)
        } else {
            // Already at beginning of buffer.
            self.buffer.borrow().get_pos()
        }
    }

    fn move_right(&mut self) -> usize {
        // Tries to move buffer position right by 1 character, though it may already be
        // at end of buffer.
        let right = self.buffer.borrow().forward().index().next();

        if let Some((cursor_pos, c)) = right {
            // Calculate new column number based on adjacent character and current
            // location of cursor.
            let col = if c == '\n' {
                0
            } else {
                if self.cursor.col < self.cols - 1 {
                    self.cursor.col + 1
                } else {
                    0
                }
            };

            // New column number at left edge of window implies that cursor wraps to
            // next row, which may require changes to canvas.
            let row = if col == 0 {
                if self.cursor.row < self.rows - 1 {
                    self.cursor.row + 1
                } else {
                    self.back.shift_up(1);
                    self.render_rows(self.cursor.row, 1, cursor_pos + 1);
                    self.draw();
                    self.cursor.row
                }
            } else {
                self.cursor.row
            };

            // Set cursor position on display.
            self.cursor = Point::new(row, col);
            self.display.write_cursor(self.cursor);
            self.display.send();

            // Set cursor position in buffer.
            self.buffer.borrow_mut().set_pos(cursor_pos + 1)
        } else {
            // Already at end of buffer.
            self.buffer.borrow().get_pos()
        }
    }

    pub fn draw(&mut self) {
        // Determine which cells changed in back canvas, if any, which then results in
        // constructing series of instructions to update display.
        let changes = self.front.reconcile(&self.back);
        if changes.len() > 0 {
            let mut last = None;

            for (p, cell) in changes {
                self.display.write_cell(p, cell, last);
                last = Some((p, cell));
            }

            self.display.write_cursor(self.cursor);
            self.display.send();
        }
    }

    pub fn redraw(&mut self) {
        self.front.clear();
        self.draw();
    }

    fn render_rows(&mut self, row: u32, rows: u32, row_pos: usize) {
        assert!(row < self.rows);
        assert!(row + rows <= self.rows);

        // Objective of this loop is to write specified range of rows to back canvas.
        let end_row = row + rows;
        let mut row = row;
        let mut col = 0;
        let blank_cell = Cell::new(' ', self.color);

        for (pos, c) in self.buffer.borrow().forward_from(row_pos).index() {
            if c == '\n' {
                let cells = self.back.row_mut(row);
                cells[(col as usize)..(self.cols as usize)].fill(blank_cell);
                col = self.cols;
            } else {
                self.back.put(row, col, Cell::new(c, self.color));
                col += 1;
            }
            if col == self.cols {
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
            let cells = self.back.row_mut(row);
            cells[(col as usize)..(self.cols as usize)].fill(blank_cell);
            row += 1;
        }
        while row < end_row {
            let cells = self.back.row_mut(row);
            cells.fill(blank_cell);
            row += 1;
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
        self.buffer.borrow().get_pos() - self.cursor.col as usize
    }

    // returns buffer position of previous row, where row_pos is assumed to be beginning of
    // current row.
    //
    // returns None if current row is already at top.
    fn find_prev_row(&self, row_pos: usize) -> Option<usize> {
        if row_pos > 0 {
            // Scans backward until \n encountered, skipping first character since it
            // may be \n if prior row did not wrap. Result identifies beginning of prior
            // line (not row), which could be larger than width of window.
            let buf = self.buffer.borrow();
            let result = buf
                .backward_from(row_pos)
                .index()
                .skip(1)
                .find(|&(_, c)| c == '\n');

            // Distance between current row position and prior line position could be
            // larger than width of window.
            let pos = match result {
                Some((pos, _)) => pos + 1,
                None => 0,
            };

            let delta = (row_pos - pos) % self.cols as usize;
            Some(row_pos - if delta > 0 {
                delta
            } else {
                self.cols as usize
            })
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
        let buf = self.buffer.borrow();
        let stop_pos = row_pos + self.cols as usize;
        let result = buf
            .forward_from(row_pos)
            .index()
            .find(|&(pos, c)| pos == stop_pos || c == '\n');

        // Note that if find operation terminates due to scanning maximum number of
        // characters, this condition indicates a soft wrap. Otherwise, implied \n
        // is skipped over.
        match result {
            Some((pos, _)) if pos == stop_pos => Some(pos),
            Some((pos, _)) => Some(pos + 1),
            None => None,
        }
    }

    // finds buffer position of specified column relative to row_pos, where row_pos is assumed
    // to be current row.
    //
    // returns tuple of (buffer position, actual column), where actual column may be less
    // than specified column.
    fn find_col(&self, row_pos: usize, col: u32) -> (usize, u32) {
        // Scans forward until \n encountered or number of characters processed reaches
        // specified column.
        let buf = self.buffer.borrow();
        let stop_pos = row_pos + col as usize;
        let result = buf
            .forward_from(row_pos)
            .index()
            .find(|&(pos, c)| pos == stop_pos || c == '\n');

        let col_pos = match result {
            Some((pos, _)) => pos,
            None => buf.size(),
        };
        (col_pos, (col_pos - row_pos) as u32)
    }

    // returns column number corresponding to current buffer position, which is
    // always <= self.cols
    fn detect_col(&self) -> u32 {
        // Scan backwards to find first \n or beginning of buffer, whichever comes first,
        // denoting beginning of line (not row).
        let buf = self.buffer.borrow();
        let result = buf
            .backward()
            .index()
            .find(|&(pos, c)| c == '\n');

        let line_pos = match result {
            Some((pos, _)) => pos + 1,
            None => 0,
        };

        // Column numnber can be easily derived based on width of window.
        (buf.get_pos() - line_pos) as u32 % self.cols
    }
}
