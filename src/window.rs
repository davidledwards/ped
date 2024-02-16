//! Window management.

use crate::ansi;
use crate::buffer::Buffer;
use crate::canvas::{Canvas, Cell, Point};
use std::cell::RefCell;
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
    rows: usize,
    cols: usize,
    origin: Point, // relative to terminal; these are physical coordinates
    cursor: Point, // relative to origin
    buffer: Rc<RefCell<Buffer>>,
    back: Canvas, // characters are written to this canvas
    front: Canvas, // reflection of what is displayed to user
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
    pub fn new(rows: usize, cols: usize, buffer: Rc<RefCell<Buffer>>) -> Window {
        assert!(rows > 0);
        assert!(cols > 0);

        Window {
            rows,
            cols,
            origin: Point::new(0, 0),
            cursor: Point::new(0, 0),
            buffer,
            back: Canvas::new(rows, cols),
            front: Canvas::new(rows, cols),
        }
    }

    // TODO: temp function
    pub fn debug_init(&mut self) {
        self.back.fill(Cell::new('-', 5, 232));
        self.front.fill(Cell::new('-', 5, 232));
    }

    // TODO: temp function
    pub fn debug_change_0(&mut self) {
        self.back.fill(Cell::new('^', 5, 232));
    }

    // TODO: temp function
    pub fn debug_change_1(&mut self) {
        let row = self.rows / 2;
        let col = self.cols / 2;
        self.back.put(row, col, Cell::new('*', 5, 232));
        self.back.put(row, col + 1, Cell::new('%', 5, 233));
        self.back.put(row + 1, col, Cell::new('^', 3, 232));
    }

    //    pub fn fill(&mut self) {
    //        self.front.fill(Cell { value: 'a', fg: 4, bg: 16 });
    //    }

    // reconciles changes from back canvas to front canvas
    // generates ANSI display sequence to send to terminal output
    pub fn refresh(&mut self) {
        let changes = self.front.reconcile(&self.back);
        if changes.len() > 0 {
            let mut output = String::new();
            let mut prev_p: Option<Point> = None;
            let mut prev_cell: Option<Cell> = None;
            for (p, cell) in changes {
                let seq = match prev_p {
                    None => {
                        Some(ansi::set_cursor(self.origin.row + p.row, self.origin.col + p.col))
                    }
                    Some(prev_p) if p.row != prev_p.row || p.col != prev_p.col + 1 => {
                        Some(ansi::set_cursor(self.origin.row + p.row, self.origin.col + p.col))
                    }
                    _ => None,
                };
                if let Some(seq) = seq {
                    output.push_str(seq.as_str());
                }

                let seq = match prev_cell {
                    None => {
                        Some(ansi::set_color(cell.fg, cell.bg))
                    }
                    Some(prev_cell) if cell.fg != prev_cell.fg || cell.bg != prev_cell.bg => {
                        Some(ansi::set_color(cell.fg, cell.bg))
                    }
                    _ => None,
                };
                if let Some(seq) = seq {
                    output.push_str(seq.as_str());
                }

                output.push(cell.value);
                prev_p = Some(p);
                prev_cell = Some(cell);
            }
            //println!("{}", output);
            println!("refresh: {:?}", output);
        }
    }

    // repaints the entire window
    //
    // in theory, a repaint could simply use the refresh() function after clearing the front
    // canvas, effectively causing each cell to be redisplayed. however, the operation can
    // be optimized because we know the front canvas needs to be entirely refreshed.
    pub fn draw(&self) {
        let mut output = String::new();
        for (row, cols) in self.front.row_iter() {
            output.push_str(ansi::set_cursor(self.origin.row + row, self.origin.col).as_str());
            let mut prev_cell = &Cell::empty();
            for (col, cell) in cols {
                if col == 0 || cell.fg != prev_cell.fg || cell.bg != prev_cell.bg {
                    output.push_str(ansi::set_color(cell.fg, cell.bg).as_str());
                }
                prev_cell = cell;
                output.push(cell.value);
            }
        }
        println!("{}", output);
    }
}
