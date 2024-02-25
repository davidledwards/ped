//! Window management.

use crate::ansi;
use crate::buffer::{BackwardIndex, Buffer};
use crate::canvas::{Canvas, Cell, Point};
use crate::color::Color;
use crate::display::Display;
use std::cell::RefCell;
use std::cmp;
use std::rc::Rc;

//
// state:
// - rows: # of rows
// - cols: # of colums
// - origin in display: (line, col)
// - origin pos: position of the origin in the buffer
// - cursor: (row, col)
// - cursor pos: position of the cursor in the buffer
//
// windows should be created by the display only, which enforces proper tiling of windows
// such that none are overlapping. when display is resized, windows are resized as well.
//
// a cursor detemines which part of the underlying buffer is displayed. all edits or movements
// are relative to the cursor.
// - ex: inserting a char
// - ex: cutting a region: the start position of the region may not be displayed, but the end
//   position would be. in this case, the cut would cause the cursor to point to a new position
//   in the buffer, then based on where we keep the cursor on the display, the origin of the
//   window (top left) would point to a new position in the buffer, and we'd repaint the
//   window.
//
// structure of window:
// - write area: vector representing the characters that should appear on the terminal, wherever
//   the window might be placed. this vector is agnostic of actual display area, just a faithful
//   representation of what should be displayed.
// - display area: vector representing what is currently being displayed. again, this is agnostic
//   of where the display area might be.
// - general flow:
//   - edits are committed to the write area
//   - updates to the display are done by comparing the write area to the display area, and
//     generating instructions that are sent to the output stream.
//   - once the display is updated, the display area is consistent with the write area.
//

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
// initializing window:
// - create the window with origin and size information, as well as the buffer.
// - the window should be drawn; since this is the first time, it is a full repaint.
// - question: where is the cursor? and where is the buffer position?
// - the buffer position may actually dictate how the window is drawn. for example,
//   if the pos is 0 (top of buffer), then the window must place the cursor at the
//   top-right of the window and draw from pos 0 at the origin. suppose the pos is
//   at the end of the buffer, say a very large file with thousands of lines. the
//   window will display the last page of the buffer, but it needs to backtrack
//   from the pos to determine where to start.
//     for (pos, c) in buf.backward_iter(pos) { ... }
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
        let (row_pos, _) = self.find_up(0);
        let (origin_pos, rows) = self.find_up(row);
        let col = (self.buffer.borrow().get_pos() - row_pos) as u32;

        // Renders the entire back canvas before drawing.
        self.render_rows(0, self.rows, origin_pos);
        self.draw();

        // Set cursor position on display.
        self.cursor = Point::new(rows, col);
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
            // otherwise just position of cursor on display changes.
            let row = if self.cursor.row > 0 {
                self.cursor.row - 1
            } else {
                self.back.shift_down(1);
                self.render_rows(0, 1, row_pos);
                0
            };
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
        0
    }

    fn move_left(&mut self) -> usize {
        0
    }

    fn move_right(&mut self) -> usize {
        0
    }

    fn move_page_down(&mut self) -> usize {
        0
    }

    // finds position of specified col or newline, whichever comes first
    // returns (buffer pos, actual col)
    fn find_col(&self, from_pos: usize, col: u32) -> (usize, u32) {
        let buf = self.buffer.borrow();
        let r = buf.forward_from(from_pos).index().find(
            |&(pos, c)| pos - from_pos == col as usize || c == '\n'
        );
        let col_pos = match r {
            Some((pos, _)) => pos,
            None => buf.size(),
        };
        (col_pos, (col_pos - from_pos) as u32)
    }

    fn render_rows(&mut self, row: u32, rows: u32, row_pos: usize) {
        assert!(row < self.rows);
        assert!(rows <= self.rows);
        assert!(row + rows <= self.rows);

        // Objective of this loop is to write specified range of rows to back canvas.
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
            if row == rows {
                break;
            }
        }

        // Blanks out any remaining cells if end of buffer is reached for all rows not yet
        // processed.
        if row < rows {
            let cells = self.back.row_mut(row);
            cells[(col as usize)..(self.cols as usize)].fill(blank_cell);
            row += 1;
        }
        while row < rows {
            let cells = self.back.row_mut(row);
            cells.fill(blank_cell);
            row += 1;
        }
    }

    fn find_up_from(&self, from_pos: usize, rows: u32) -> (usize, u32) {
        // Keep track of actual number of rows processed, which may be less than
        // requested number.
        let mut row = rows;

        // Objective of this loop is to find buffer position corresponding to beginning
        // of row based on number of rows prior to specified buffer position, taking into
        // account line wrapping.
        let buf = self.buffer.borrow();
        let mut it = buf.backward_from(from_pos).index();
        let mut buf_pos = from_pos;
        let mut bol_pos = find_bol(&mut it);

        let bor_pos = loop {
            // Remaining distance between current buffer position and position of closest
            // beginning of line.
            let pos_diff = buf_pos - bol_pos;

            // Buffer position is decremented only as much as number of columns managed by
            // window, essentially accounting for long lines.
            buf_pos -= {
                if pos_diff < self.cols as usize {
                    pos_diff
                } else {
                    let n = pos_diff % (self.cols as usize);
                    if n > 0 {
                        n
                    } else {
                        self.cols as usize
                    }
                }
            };

            // Once number of ideal rows has been processed or beginning of buffer is
            // reached, current buffer positon corresponds to beginning of row.
            if row == 0 || buf_pos == 0 {
                break buf_pos;
            } else {
                row -= 1;
            }

            // Once current buffer position has reached beginning of line, scan backwards
            // to find next beginning of line.
            if buf_pos == bol_pos {
                // Current buffer position points to first character of line, but calculation
                // is based on pointing to '\n', so just subtract 1.
                buf_pos -= 1;
                bol_pos = find_bol(&mut it);
            }
        };
        (bor_pos, rows - row)
    }

    fn find_up(&self, rows: u32) -> (usize, u32) {
        self.find_up_from(self.buffer.borrow().get_pos(), rows)
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
}

// Scans backwards until next '\n' character is found and returns buffer position
// of character that follows, or returns 0 if beginning of buffer is reached.
fn find_bol(it: &mut BackwardIndex) -> usize {
    match it.find(|&(_, c)| c == '\n') {
        Some((pos, _)) => pos + 1,
        None => 0,
    }
}
