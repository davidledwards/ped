//! Provides text rendering engines.

use crate::buffer::BufferRef;
use crate::config::ConfigurationRef;
use crate::nav::Location;
use crate::size::{Point, Size};
use crate::token::{Cursor, Tokenizer};
use crate::window::WindowRef;
use std::ops::Range;

/// A trait that all rendering engines are required to implement.
pub trait Renderer {
    /// Returns the kind of renderer.
    fn kind(&self) -> Rendering;

    /// Attaches `window` to this renderer.
    fn attach(&mut self, window: WindowRef);

    /// Detaches the existing window from this renderer, replacing it with a _zombie_
    /// window.
    fn detach(&mut self);

    /// Returns `true` if a normal window is attached or `false` if the attached window
    /// is a _zombie_.
    fn is_attached(&self) -> bool;

    /// Returns the number of rows on the display.
    fn rows(&self) -> u32;

    /// Returns the size of the display.
    fn size(&self) -> Size;

    /// Returns the buffer position corresponding to the [`cursor`](Self::cursor).
    fn pos(&self) -> usize;

    /// Returns the cursor position on the display in terms of _row_ and _column_.
    ///
    /// The _row_ and _column_ values are `0`-based and exclusively bounded by
    /// [`size()`](Self::size).
    fn cursor(&self) -> Point;

    /// Returns the location of the cursor position in the buffer.
    fn location(&self) -> Location;

    /// Returns the starting (_inclusive_) and ending (_exclusive_) buffer positions
    /// of the line occupied by the cursor.
    fn line(&self) -> (usize, usize);

    /// Returns the starting (_inclusive_) and ending (_exclusive_) buffer positions
    /// of the line at the top of the display.
    fn top(&self) -> (usize, usize);

    /// Sets the cursor location and corresponding buffer position to `cursor`, though
    /// the final cursor location is constrained by end-of-line and end-of-buffer
    /// boundaries.
    ///
    /// This function was designed for responding to _mouse click_ events where the
    /// position of the click is captured in `cursor`.
    ///
    /// The coordinates in `cursor` are presumed to be relative to the origin of the
    /// editor canvas.
    fn focus_cursor(&mut self, cursor: Point);

    /// Sets the position of the cursor based on the alignment objective `align`.
    fn align_cursor(&mut self, align: Align);

    /// Makes the cursor visible on display.
    fn show_cursor(&mut self);

    /// Tries to move the cursor _up_ by the specified number of `try_rows`,
    /// returning the actual number of rows moved.
    ///
    /// If `pin` is `true`, then the cursor will remain on the current row if the
    /// resulting display makes it possible. Pinning is useful when _paging up_.
    ///
    /// If `pin` is `false`, then the cursor will move up in tandem with `try_rows`,
    /// though not to extend beyond the top of the display.
    fn move_up(&mut self, try_rows: u32, pin: bool) -> u32;

    /// Tries to move the cursor _down_ by the specified number of `try_rows`,
    /// returning the actual number of rows moved.
    ///
    /// If `pin` is `true`, then the cursor will remain on the current row. Pinning is
    /// useful when _paging down_.
    ///
    /// If `pin` is `false`, then the cursor will move down in tandem with `try_rows`,
    /// though not to extend beyond the bottom of the display.
    fn move_down(&mut self, try_rows: u32, pin: bool) -> u32;

    /// Moves the cursor to the _start_ of the current row.
    fn move_start(&mut self);

    /// Moves the cursor to the _end_ of the current row.
    fn move_end(&mut self);

    /// Moves the current buffer position to `pos` and places the cursor on the
    /// display according to the `align` objective.
    ///
    /// When [`Align::Auto`] is specified, the placement of the cursor depends on
    /// the target `pos` relative to the current buffer position. Specifically, it
    /// behaves as follows:
    /// - _when `pos` is above the current row but still visible on the display_:
    ///   aligns the cursor on the target row above the current row, though not to
    ///   extend beyond the top row
    /// - _when `pos` is on the current row_: aligns the cursor on the current row
    /// - _when `pos` is beyond the current row_: aligns the cursor on the target
    ///   row below the current row, though not to extend beyond the bottom row
    fn move_to(&mut self, pos: usize, align: Align);

    /// Tries scrolling _up_ the contents of the display by the specified number of
    /// `try_rows` while preserving the cursor position, which also means the cursor
    /// moves _up_ as the contents scroll, returning the actual number of rows
    /// scrolled.
    fn scroll_up(&mut self, try_rows: u32) -> u32;

    /// Tries scrolling _down_ the contents of the display by the specified number of
    /// `try_rows` while preserving the cursor position, which also means the cursor
    /// moves _down_ as the contents scroll, returning the actual number of rows
    /// scrolled.
    fn scroll_down(&mut self, try_rows: u32) -> u32;

    /// Moves the current buffer position and cursor location to reflect the insertion
    /// of `len` characters at the current buffer position.
    ///
    /// This function assumes that the text has already been inserted into the buffer as
    /// a precondition.
    fn insert(&mut self, len: usize);

    /// Moves the cursor location to reflect the removal of characters at the current
    /// buffer position.
    ///
    /// This function assumes that the text has already been removed from the buffer as
    /// a precondition.
    fn remove(&mut self);

    /// Renders the display.
    fn render(&mut self, tok: &Tokenizer, syn: Cursor, selected: Option<Range<usize>>);
}

/// The types of rendering engines.
pub enum Rendering {
    /// A rendering engine that wraps lines exceeding the width of the display.
    Wrapping,

    /// A rendering engine the scrolls text horizontally for lines exceeding the width of
    /// the display.
    Scrolling,
}

/// Cursor alignment directives.
pub enum Align {
    /// Try aligning the cursor based on its contextual use.
    Auto,

    /// Try aligning the cursor in the center of the window.
    Center,

    /// Try aligning the cursor at the top of the window.
    Top,

    /// Try aligning the cursor at the bottom of the window.
    Bottom,

    /// Try aligning the cursot at the specified row.
    Row(u32),
}

impl Rendering {
    /// Create a new rendering engine.
    pub fn new(&self, config: ConfigurationRef, buffer: BufferRef) -> Box<dyn Renderer> {
        match self {
            Self::Wrapping => Box::new(wrapping::Engine::new(config, buffer)),
            Self::Scrolling => Box::new(scrolling::Engine::new(config, buffer)),
        }
    }
}

/// Number of columns allocated to the margin.
const MARGIN_COLS: u32 = 6;

/// Upper bound (exclusive) on line numbers that can be displayed in the margin.
const LINE_LIMIT: u32 = 10_u32.pow(MARGIN_COLS - 1);

/// Number of columns used to display lower-order digits of line numbers that
/// must be clipped when larger than `LINE_LIMIT`.
const CLIP_LOWER_COLS: u32 = MARGIN_COLS / 2;

/// Number of columns used to hide higher-order digits of line numbers that
/// must be clipped when larger than `LINE_LIMIT`.
const CLIP_UPPER_COLS: u32 = MARGIN_COLS - CLIP_LOWER_COLS - 1;

/// This module implements the [wrapping](Rendering::Wrapping) engine.
mod wrapping {
    use super::*;
    use crate::buffer::Buffer;
    use crate::canvas::Canvas;
    use crate::nav;
    use crate::style::{Pen, Styler};
    use crate::window::Window;
    use std::cell::Ref;
    use std::cmp;

    pub struct Engine {
        /// Global configuration.
        config: ConfigurationRef,

        /// Underlying buffer used for navigation and rendering.
        buffer: BufferRef,

        /// Buffer position that corresponds to the cursor position.
        pos: usize,

        /// Styler used to create pens for rendering.
        styler: Styler,

        /// Window for rendering contents of the buffer.
        window: WindowRef,

        /// Number of rows for displaying text.
        rows: u32,

        /// Number of columns for displaying text.
        cols: u32,

        /// Indicates when the left margin is enabled.
        margin_enabled: bool,

        /// Top row of the display.
        top_row: Row,

        /// Cursor row of the display.
        cur_row: Row,

        /// An optional column to which the cursor should _snap_ when moving up and down.
        snap: Option<u32>,

        /// Position of the cursor on the display.
        cursor: Point,
    }

    /// A _row_ on the display that corresponds to a _line_ in the buffer.
    ///
    /// A row should not be confused with a line in the buffer, the latter of which
    /// could conceivably span more than one row on the display.
    #[derive(Copy, Clone)]
    struct Row {
        /// Buffer position corresponding to the first character of the display row,
        /// which is always greater than or equal to `line_pos`.
        row_pos: usize,

        /// Length of the display row, including the `\n` if one exists.
        row_len: usize,

        /// Buffer position corresponding to the first character of the buffer line.
        line_pos: usize,

        /// Length of the buffer line, including the `\n` if one exists.
        line_len: usize,

        /// The `0`-based number of the buffer line.
        line: u32,

        /// Indicates that the buffer line is the bottom-most line in the buffer.
        is_bottom: bool,

        /// Width of display.
        cols: u32,
    }

    // Represents a point on the display and additional state used during the rendering
    // process.
    struct Spot<'a> {
        /// Current rendering position in the buffer.
        pos: usize,

        /// Current rendering row.
        row: u32,

        /// Current rendering column.
        col: u32,

        /// Current rendering line number, which is `1`-based.
        line: u32,

        /// Indicates that the buffer line has wrapped.
        line_wrapped: bool,

        /// Reference to the tokenizer used for syntax coloring.
        ///
        /// This value does not change during the rendering process.
        tok: &'a Tokenizer,

        /// Current rendering syntax cursor.
        syn: Cursor,
    }

    impl Row {
        /// Returns a possibly smaller value of `col` if it extends beyond the end of
        /// the row.
        ///
        /// In most cases, the right-most column aligns to the last character of the row,
        /// which is usually `\n` but may also be any other character if the row wraps.
        /// However, if this is the bottom-most row in the buffer, there is no terminating
        /// `\n`, and thus the right-most column is right of the last character.
        fn snap(&self, col: u32) -> u32 {
            if self.row_len == 0 {
                0
            } else if self.is_bottom() {
                cmp::min(col, self.row_len as u32)
            } else {
                cmp::min(col, self.row_len as u32 - 1)
            }
        }

        /// Returns the buffer position of `col` relative to the starting position of
        /// the row.
        #[inline]
        fn pos_of(&self, col: u32) -> usize {
            self.row_pos + (col as usize)
        }

        /// Returns the column number of `pos` relative to the starting position of the
        /// row.
        #[inline]
        fn col_of(&self, pos: usize) -> u32 {
            (pos - self.row_pos) as u32
        }

        /// Returns the buffer position of the right-most column number of this row.
        #[inline]
        fn end_col_pos(&self) -> usize {
            self.row_pos
                + if self.is_bottom() {
                    self.row_len
                } else {
                    self.row_len - 1
                }
        }

        /// Returns the buffer position at the end of the row.
        #[inline]
        fn end_pos(&self) -> usize {
            self.row_pos + self.row_len
        }

        /// Returns `true` if the line associated with this row points to the bottom of
        /// the buffer.
        #[inline]
        fn is_bottom(&self) -> bool {
            self.is_bottom && self.row_len < (self.cols as usize)
        }

        /// Returns `true` if the row of this line wraps at least to the next row,
        /// indicating that the buffer line is longer than the width of the display.
        #[inline]
        fn is_wrapping(&self) -> bool {
            self.row_pos + self.row_len < self.line_pos + self.line_len
        }

        /// Moves _up_ by `try_rows` relative to this row, or possibly fewer rows if the
        /// top of the buffer is reached, returning a pair containing the resulting row
        /// and the actual number of rows moved.
        fn up(&self, buffer: &Buffer, try_rows: u32) -> (Row, u32) {
            let mut row = *self;
            for rows in 0..try_rows {
                if let Some(r) = row.prev(buffer) {
                    row = r;
                } else {
                    return (row, rows);
                }
            }
            (row, try_rows)
        }

        /// Moves _down_ by `try_rows` relative to this row, or possibly fewer rows if the
        /// bottom of the buffer is reached, returning a pair containing the resulting row
        /// and the actual number of rows moved.
        fn down(&self, buffer: &Buffer, try_rows: u32) -> (Row, u32) {
            let mut row = *self;
            for rows in 0..try_rows {
                if let Some(r) = row.next(buffer) {
                    row = r
                } else {
                    return (row, rows);
                }
            }
            (row, try_rows)
        }

        /// Finds the row _up_ from this row that contains `pos`, returning a pair
        /// containing the resulting row and the total number of rows moved.
        fn find_up(&self, buffer: &Buffer, pos: usize) -> (Row, u32) {
            let mut row = *self;
            let mut rows = 0;
            while pos < row.row_pos {
                row = row.prev_unchecked(buffer);
                rows += 1;
            }
            (row, rows)
        }

        /// Finds the row _down_ from this row that contains `pos`, returning a pair
        /// containing the resulting row and the total number of rows moved.
        fn find_down(&self, buffer: &Buffer, pos: usize) -> (Row, u32) {
            let mut row = *self;
            let mut rows = 0;
            while pos >= row.end_pos() && !row.is_bottom {
                row = row.next_unchecked(buffer);
                rows += 1;
            }
            (row, rows)
        }

        /// Returns an update version of this row based on the assumption of underlying
        /// changes to the buffer.
        ///
        /// Note that none of `row_pos`, `line_pos`, and `line` are modified as part of
        /// this update, as those are assumed to be unchanged.
        ///
        /// The rationale for this function is that an insertion or deletion of text is
        /// always relative to the current row, and that such a change would never
        /// alter the values noted above.
        fn update(&self, buffer: &Buffer) -> Row {
            let (next_pos, is_bottom) = nav::find_next_line(buffer, self.line_pos);
            let line_len = next_pos - self.line_pos;
            let row_len = cmp::min(
                line_len - (self.row_pos - self.line_pos),
                self.cols as usize,
            );
            Row {
                row_len,
                line_len,
                is_bottom,
                ..*self
            }
        }

        /// Returns the row preceding this row, or `None` if this row is already at the
        /// top of the buffer.
        fn prev(&self, buffer: &Buffer) -> Option<Row> {
            if self.row_pos == 0 {
                None
            } else if self.row_pos > self.line_pos {
                let r = Row {
                    row_pos: self.row_pos - (self.cols as usize),
                    row_len: self.cols as usize,
                    ..*self
                };
                Some(r)
            } else {
                let pos = self.line_pos - 1;
                let (line_pos, next_pos, is_bottom) = Self::find_row_bounds(buffer, pos);
                let line_len = next_pos - line_pos;
                let row_pos = pos - ((pos - line_pos) % (self.cols as usize));
                let row_len = cmp::min(line_len - (row_pos - line_pos), self.cols as usize);
                let r = Row {
                    row_pos,
                    row_len,
                    line_pos,
                    line_len,
                    line: self.line - 1,
                    is_bottom,
                    ..*self
                };
                Some(r)
            }
        }

        /// An unchecked version of [`prev`](Self::prev) that assumes this row is not at the
        /// top of the buffer.
        fn prev_unchecked(&self, buffer: &Buffer) -> Row {
            self.prev(buffer)
                .unwrap_or_else(|| panic!("row already at top of buffer"))
        }

        /// Returns the row following this row, or `None` if this row is already at the
        /// bottom of the buffer.
        fn next(&self, buffer: &Buffer) -> Option<Row> {
            if self.is_bottom() {
                None
            } else if self.is_wrapping() {
                let row_pos = self.row_pos + self.row_len;
                let row_len = cmp::min(
                    self.line_len - (row_pos - self.line_pos),
                    self.cols as usize,
                );
                let r = Row {
                    row_pos,
                    row_len,
                    ..*self
                };
                Some(r)
            } else {
                let line_pos = self.line_pos + self.line_len;
                let (next_pos, is_bottom) = nav::find_next_line(buffer, line_pos);
                let line_len = next_pos - line_pos;
                let row_len = cmp::min(line_len, self.cols as usize);
                let r = Row {
                    row_pos: line_pos,
                    row_len,
                    line_pos,
                    line_len,
                    line: self.line + 1,
                    is_bottom,
                    ..*self
                };
                Some(r)
            }
        }

        /// An unchecked version of [`next`](Self::next) that assumes this row is not at
        /// the top of the buffer.
        fn next_unchecked(&self, buffer: &Buffer) -> Row {
            self.next(buffer)
                .unwrap_or_else(|| panic!("row already at bottom of buffer"))
        }

        /// Finds and returns the row that contains `pos`.
        ///
        /// This function is expensive because it must calculate the line number
        /// corresponding to `pos` by performing a linear scan from the beginning of the
        /// buffer.
        fn find_row(buffer: &Buffer, pos: usize, cols: u32) -> Row {
            let (line_pos, next_pos, is_bottom) = Self::find_row_bounds(buffer, pos);
            let line_len = next_pos - line_pos;
            let row_pos = pos - ((pos - line_pos) % (cols as usize));
            let row_len = cmp::min(line_len - (row_pos - line_pos), cols as usize);
            Row {
                row_pos,
                row_len,
                line_pos,
                line_len,
                line: nav::find_location(buffer, line_pos).line,
                is_bottom,
                cols,
            }
        }

        /// Returns a tuple, relative to the line in `buffer` corresponding to `pos`,
        /// containing the position of the first character on that line, the position of
        /// the first character of the next line, and a boolean value indicating if the
        /// end of buffer has been reached.
        fn find_row_bounds(buffer: &Buffer, pos: usize) -> (usize, usize, bool) {
            let line_pos = nav::find_start_line(buffer, pos);
            let (next_pos, is_bottom) = nav::find_next_line(buffer, pos);
            (line_pos, next_pos, is_bottom)
        }
    }

    #[allow(clippy::derivable_impls, reason = "retain expressiveness")]
    impl Default for Row {
        fn default() -> Row {
            Row {
                row_pos: 0,
                row_len: 0,
                line_pos: 0,
                line_len: 0,
                line: 0,
                is_bottom: false,
                cols: 0,
            }
        }
    }

    impl<'a> Spot<'a> {
        /// Creates a spot representing the top-left position on the display.
        fn new(engine: &Engine, tok: &'a Tokenizer, syn: Cursor) -> Spot<'a> {
            Spot {
                pos: engine.top_row.row_pos,
                row: 0,
                col: 0,
                line: engine.top_row.line + 1,
                line_wrapped: false,
                tok,
                syn,
            }
        }

        fn next_col(mut self) -> Spot<'a> {
            self.pos += 1;
            self.col += 1;
            self.syn = self.tok.forward(self.syn, 1);
            self
        }

        fn next_row(mut self) -> Spot<'a> {
            self.pos += 1;
            self.row += 1;
            self.col = 0;
            self.line_wrapped = true;
            self.syn = self.tok.forward(self.syn, 1);
            self
        }

        fn next_line(mut self) -> Spot<'a> {
            self.pos += 1;
            self.row += 1;
            self.col = 0;
            self.line += 1;
            self.line_wrapped = false;
            self.syn = self.tok.forward(self.syn, 1);
            self
        }
    }

    impl Engine {
        /// Creates a new rendering engine with an unattached window using `config`
        /// and `buffer`.
        pub fn new(config: ConfigurationRef, buffer: BufferRef) -> Engine {
            let pos = buffer.borrow().get_pos();
            let styler = Styler::new(config.clone());

            Engine {
                config,
                buffer,
                pos,
                styler,
                window: Window::zombie().into_ref(),
                rows: 0,
                cols: 0,
                margin_enabled: false,
                top_row: Row::default(),
                cur_row: Row::default(),
                snap: None,
                cursor: Point::ORIGIN,
            }
        }

        /// Renders an individual cell for the character `c`, returning the next rendering
        /// context or `None` if rendering has finished.
        fn render_cell<'a>(
            &self,
            pen: &Pen,
            spot: Spot<'a>,
            c: char,
            canvas: &mut Canvas,
        ) -> Option<Spot<'a>> {
            self.render_margin(pen, &spot, canvas);
            let (row, col) = (spot.row, spot.col + self.margin_cols());
            let color = spot.syn.color();
            let spot = if c == '\n' {
                canvas.set_cell(row, col, pen.as_text(c, spot.pos, row, color));
                canvas.fill_cell_from(row, col + 1, pen.as_text(' ', spot.pos, row, color));
                spot.next_line()
            } else {
                canvas.set_cell(row, col, pen.as_text(c, spot.pos, row, color));
                if spot.col + 1 < self.cols {
                    spot.next_col()
                } else {
                    spot.next_row()
                }
            };
            if spot.row < self.rows {
                Some(spot)
            } else {
                None
            }
        }

        /// Renders the remainder of the displayable area which is considered empty space.
        ///
        /// This function gets invoked when the end of buffer is reached before the entire
        /// canvas is rendered.
        fn render_rest(&self, pen: &Pen, spot: Spot, canvas: &mut Canvas) {
            self.render_margin(pen, &spot, canvas);

            // Blank out rest of existing row.
            let (row, col) = (spot.row, spot.col + self.margin_cols());
            let color = spot.syn.color();
            canvas.fill_cell_from(row, col, pen.as_text(' ', spot.pos, spot.row, color));

            // Blank out remaining rows.
            for row in (spot.row + 1)..self.rows {
                if self.margin_enabled {
                    canvas.fill_cell(row, 0..MARGIN_COLS, pen.as_margin(' '));
                }
                canvas.fill_cell_from(row, MARGIN_COLS, pen.as_blank());
            }
        }

        /// Renders the margin if line numbering is enabled and the rendering context is
        /// on the first column of any row.
        fn render_margin(&self, pen: &Pen, spot: &Spot, canvas: &mut Canvas) {
            if spot.col == 0 && self.margin_enabled {
                if spot.line_wrapped {
                    canvas.fill_cell(spot.row, 0..MARGIN_COLS, pen.as_margin(' '));
                } else if spot.line < LINE_LIMIT {
                    let s = format!("{:>cols$} ", spot.line, cols = MARGIN_COLS as usize - 1);
                    for (col, c) in s.char_indices() {
                        canvas.set_cell(spot.row, col as u32, pen.as_line(c, spot.line));
                    }
                } else {
                    canvas.fill_cell(spot.row, 0..CLIP_UPPER_COLS, pen.as_line('-', spot.line));
                    let s = format!(
                        "{:0>cols$}",
                        spot.line % 10_u32.pow(CLIP_LOWER_COLS),
                        cols = CLIP_LOWER_COLS as usize,
                    );
                    for (col, c) in s.char_indices() {
                        canvas.set_cell(
                            spot.row,
                            col as u32 + CLIP_UPPER_COLS,
                            pen.as_line(c, spot.line),
                        );
                    }
                    canvas.set_cell(spot.row, MARGIN_COLS, pen.as_margin(' '));
                }
            }
        }

        /// Moves `cur_row` _up_ by `try_rows` or possibly fewer if the top of the buffer
        /// is reached, returning the actual number of rows moved.
        fn up_cur_row(&mut self, try_rows: u32) -> u32 {
            let (cur_row, rows) = self.cur_row.up(&self.buffer(), try_rows);
            self.cur_row = cur_row;
            rows
        }

        /// Moves `cur_row` _down_ by `try_rows` or possibly fewer if the bottom of the
        /// buffer is reached, returning the actual number of rows moved.
        fn down_cur_row(&mut self, try_rows: u32) -> u32 {
            let (cur_row, rows) = self.cur_row.down(&self.buffer(), try_rows);
            self.cur_row = cur_row;
            rows
        }

        /// Moves `top_row` _up_ by `try_rows` or possibly fewer if the top of the buffer
        /// is reached, returning the actual number of rows moved.
        fn up_top_row(&mut self, try_rows: u32) -> u32 {
            let (top_row, rows) = self.top_row.up(&self.buffer(), try_rows);
            self.top_row = top_row;
            rows
        }

        /// Moves `top_row` _down_ by `try_rows` or possibly fewer if the bottom of the
        /// buffer is reached, returning the actual number of rows moved.
        fn down_top_row(&mut self, try_rows: u32) -> u32 {
            let (top_row, rows) = self.top_row.down(&self.buffer(), try_rows);
            self.top_row = top_row;
            rows
        }

        /// Sets `top_row` to `try_rows` before `cur_row` or possibly fewer if the top of
        /// the buffer is reached, returning the actual number of rows moved.
        fn set_top_row(&mut self, try_rows: u32) -> u32 {
            let (top_row, rows) = self.cur_row.up(&self.buffer(), try_rows);
            self.top_row = top_row;
            rows
        }

        /// Updates `cur_row` based on the assumption of underlying changes to the buffer.
        fn update_cur_row(&mut self) {
            let cur_row = self.cur_row.update(&self.buffer());
            self.cur_row = cur_row;
        }

        /// Updates `top_row` based on the assumption of underlying changes to the buffer.
        fn update_top_row(&mut self) {
            let top_row = self.top_row.update(&self.buffer());
            self.top_row = top_row;
        }

        /// Moves `cur_row` _up_ until the resulting line contains `pos`, returning the
        /// total number of rows moved.
        fn find_up_cur_row(&mut self, pos: usize) -> u32 {
            let (cur_row, rows) = self.cur_row.find_up(&self.buffer(), pos);
            self.cur_row = cur_row;
            rows
        }

        /// Moves `cur_row` _down_ until the resulting line contains `pos`, returning the
        /// total number of rows moved.
        fn find_down_cur_row(&mut self, pos: usize) -> u32 {
            let (cur_row, rows) = self.cur_row.find_down(&self.buffer(), pos);
            self.cur_row = cur_row;
            rows
        }

        #[inline]
        fn margin_cols(&self) -> u32 {
            if self.margin_enabled { MARGIN_COLS } else { 0 }
        }

        #[inline]
        fn buffer(&self) -> Ref<'_, Buffer> {
            self.buffer.borrow()
        }
    }

    impl Renderer for Engine {
        fn kind(&self) -> Rendering {
            Rendering::Wrapping
        }

        fn attach(&mut self, window: WindowRef) {
            // Allocate leftmost columns of window to line numbers, but only if enabled and
            // total width of window is large enough to reasonably accommodate.
            self.window = window;
            let Size { rows, cols } = self.window.borrow().canvas.borrow().size();
            self.margin_enabled = self.config.settings.lines && cols >= MARGIN_COLS * 2;
            self.rows = rows;
            self.cols = cols - self.margin_cols();
        }

        fn detach(&mut self) {
            self.attach(Window::zombie().into_ref());
        }

        fn is_attached(&self) -> bool {
            !self.window.borrow().is_zombie()
        }

        #[inline(always)]
        fn rows(&self) -> u32 {
            self.rows
        }

        fn size(&self) -> Size {
            Size::new(self.rows, self.cols)
        }

        #[inline(always)]
        fn pos(&self) -> usize {
            self.pos
        }

        #[inline(always)]
        fn cursor(&self) -> Point {
            self.cursor
        }

        #[inline(always)]
        fn location(&self) -> Location {
            Location::new(
                self.cur_row.line,
                (self.cur_row.row_pos - self.cur_row.line_pos) as u32 + self.cursor.col,
            )
        }

        #[inline]
        fn line(&self) -> (usize, usize) {
            (
                self.cur_row.line_pos,
                self.cur_row.line_pos + self.cur_row.line_len,
            )
        }

        #[inline]
        fn top(&self) -> (usize, usize) {
            (
                self.top_row.line_pos,
                self.top_row.line_pos + self.cur_row.line_len,
            )
        }

        fn focus_cursor(&mut self, cursor: Point) {
            // Ensure target cursor is bounded by effective area of canvas, which takes
            // into account left margin if enabled.
            let try_row = cmp::min(cursor.row, self.rows);
            let try_col = if cursor.col < self.margin_cols() {
                0
            } else {
                cmp::min(cursor.col - self.margin_cols(), self.cols)
            };

            // Find effective cursor location and buffer position by moving down from
            // top row of display.
            let (cur_row, row) = self.top_row.down(&self.buffer(), try_row);
            self.cur_row = cur_row;
            let col = self.cur_row.snap(try_col);
            self.pos = self.cur_row.pos_of(col);
            self.snap = Some(col);
            self.cursor = Point::new(row, col);
        }

        fn align_cursor(&mut self, align: Align) {
            // Determine ideal row where cursor would like to be focused, though this should
            // be considered a hint.
            let try_row = match align {
                Align::Auto => cmp::min(self.cursor.row, self.rows - 1),
                Align::Center => self.rows / 2,
                Align::Top => 0,
                Align::Bottom => self.rows - 1,
                Align::Row(row) => cmp::min(row, self.rows - 1),
            };

            // Tries to position cursor on target row, but no guarantee depending on proximity
            // of row to top of buffer.
            let cur_row = Row::find_row(&self.buffer(), self.pos, self.cols);
            self.cur_row = cur_row;
            let (top_row, row) = self.cur_row.up(&self.buffer(), try_row);
            self.top_row = top_row;
            let col = self.cur_row.col_of(self.pos);
            self.snap = None;
            self.cursor = Point::new(row, col);
        }

        fn show_cursor(&mut self) {
            let cursor = if self.margin_enabled {
                self.cursor + Size::cols(MARGIN_COLS)
            } else {
                self.cursor
            };
            self.window.borrow().canvas.borrow_mut().set_cursor(cursor);
        }

        fn move_up(&mut self, try_rows: u32, pin: bool) -> u32 {
            let rows = self.up_cur_row(try_rows);
            if rows > 0 {
                let row = if pin {
                    if rows < try_rows {
                        // Cursor reached top of buffer before advancing by desired number of
                        // rows, so resulting row is always top of display.
                        self.set_top_row(0)
                    } else {
                        // Try finding new top line by stepping backwards by number of rows
                        // equivalent to current row of cursor.
                        self.set_top_row(self.cursor.row)
                    }
                } else if rows > self.cursor.row {
                    // Cursor would have moved beyond top of display.
                    self.set_top_row(0)
                } else {
                    // Cursor remains visible without changing top line.
                    self.cursor.row - rows
                };
                let try_col = self.snap.take().unwrap_or(self.cursor.col);
                let col = self.cur_row.snap(try_col);
                self.pos = self.cur_row.pos_of(col);
                self.snap = Some(try_col);
                self.cursor = Point::new(row, col);
            }
            rows
        }

        fn move_down(&mut self, try_rows: u32, pin: bool) -> u32 {
            let rows = self.down_cur_row(try_rows);
            if rows > 0 {
                let row = if pin {
                    // Keeping cursor on current row is guaranteed, because top line can
                    // always move down without reaching bottom of buffer.
                    let _ = self.down_top_row(rows);
                    self.cursor.row
                } else if self.cursor.row + rows < self.rows {
                    // Cursor remains visible without changing top line.
                    self.cursor.row + rows
                } else {
                    // Cursor would have moved beyond bottom of display.
                    self.set_top_row(self.rows - 1)
                };
                let try_col = self.snap.take().unwrap_or(self.cursor.col);
                let col = self.cur_row.snap(try_col);
                self.pos = self.cur_row.pos_of(col);
                self.snap = Some(try_col);
                self.cursor = Point::new(row, col);
            }
            rows
        }

        fn move_start(&mut self) {
            self.move_to(self.cur_row.row_pos, Align::Row(self.cursor.row));
        }

        fn move_end(&mut self) {
            self.move_to(self.cur_row.end_col_pos(), Align::Row(self.cursor.row));
        }

        fn move_to(&mut self, pos: usize, align: Align) {
            let row = if pos < self.top_row.row_pos {
                let _ = self.find_up_cur_row(pos);
                let rows = match align {
                    Align::Top | Align::Auto => 0,
                    Align::Center => self.rows / 2,
                    Align::Bottom => self.rows - 1,
                    Align::Row(row) => cmp::min(row, self.rows - 1),
                };
                self.set_top_row(rows)
            } else if pos < self.cur_row.row_pos {
                let row = self.cursor.row - self.find_up_cur_row(pos);
                let maybe_rows = match align {
                    Align::Auto => None,
                    Align::Top => Some(0),
                    Align::Center => Some(self.rows / 2),
                    Align::Bottom => Some(self.rows - 1),
                    Align::Row(row) => Some(cmp::min(row, self.rows - 1)),
                };
                if let Some(rows) = maybe_rows {
                    self.set_top_row(rows)
                } else {
                    row
                }
            } else if pos < self.cur_row.end_pos() {
                let maybe_rows = match align {
                    Align::Auto => None,
                    Align::Top => Some(0),
                    Align::Center => Some(self.rows / 2),
                    Align::Bottom => Some(self.rows - 1),
                    Align::Row(row) => Some(cmp::min(row, self.rows - 1)),
                };
                if let Some(rows) = maybe_rows {
                    self.set_top_row(rows)
                } else {
                    self.cursor.row
                }
            } else {
                let rows = self.find_down_cur_row(pos);
                let row = match align {
                    Align::Auto => cmp::min(self.cursor.row + rows, self.rows - 1),
                    Align::Top => 0,
                    Align::Center => self.rows / 2,
                    Align::Bottom => self.rows - 1,
                    Align::Row(row) => cmp::min(row, self.rows - 1),
                };
                self.set_top_row(row)
            };
            self.pos = pos;
            let col = self.cur_row.col_of(self.pos);
            self.snap = None;
            self.cursor = Point::new(row, col);
        }

        fn scroll_up(&mut self, try_rows: u32) -> u32 {
            let rows = self.down_top_row(try_rows);
            if rows > 0 {
                let (row, col) = if rows > self.cursor.row {
                    // Cursor would have moved beyond top of display, which means current
                    // buffer position changes accordingly.
                    self.cur_row = self.top_row;
                    let try_col = self.snap.take().unwrap_or(self.cursor.col);
                    self.snap = Some(try_col);
                    let col = self.cur_row.snap(try_col);
                    self.pos = self.cur_row.pos_of(col);
                    (0, col)
                } else {
                    // Cursor still visible on display.
                    (self.cursor.row - rows, self.cursor.col)
                };
                self.cursor = Point::new(row, col);
            }
            rows
        }

        fn scroll_down(&mut self, try_rows: u32) -> u32 {
            let rows = self.up_top_row(try_rows);
            if rows > 0 {
                let row = self.cursor.row + rows;
                let (row, col) = if row < self.rows {
                    // Cursor still visible on display.
                    (row, self.cursor.col)
                } else {
                    // Cursor would have moved beyond bottom of display, which means current
                    // buffer position changes accordingly.
                    let _ = self.up_cur_row(row - self.rows + 1);
                    let try_col = self.snap.take().unwrap_or(self.cursor.col);
                    self.snap = Some(try_col);
                    let col = self.cur_row.snap(try_col);
                    self.pos = self.cur_row.pos_of(col);
                    (self.rows - 1, col)
                };
                self.cursor = Point::new(row, col);
            }
            rows
        }

        fn insert(&mut self, len: usize) {
            if len > 0 {
                // Buffer position moves forward.
                self.pos += len;

                // Update current row since insertion will changed boundaries for future
                // navigation.
                self.update_cur_row();

                // Find possibly new current row.
                let row = self.cursor.row + self.find_down_cur_row(self.pos);

                // New current row could extend beyond display boundary, so update or
                // find new top row.
                let row = if row < self.rows {
                    self.update_top_row();
                    row
                } else {
                    self.set_top_row(self.rows - 1)
                };
                let col = self.cur_row.col_of(self.pos);
                self.snap = None;
                self.cursor = Point::new(row, col);
            }
        }

        fn remove(&mut self) {
            // Removal of text requires both current row and top row to be updated since
            // boundaries may have changed.
            self.update_cur_row();
            self.update_top_row();
            let col = self.cur_row.col_of(self.pos);
            self.snap = None;
            self.cursor.col = col;
        }

        fn render(&mut self, tok: &Tokenizer, syn: Cursor, selected: Option<Range<usize>>) {
            // Create pen to format characters.
            let pen = self
                .styler
                .pen(self.cursor, self.top_row.line + 1, selected);

            // Initialize spot representing top-left cell.
            let spot = Spot::new(self, tok, syn);

            // Borrow canvas once and pass to rendering functions to optimize.
            let window = self.window.borrow();
            let mut canvas = window.canvas.borrow_mut();

            // Render cells until end of display or end of buffer reached.
            let rest = self
                .buffer()
                .forward(spot.pos)
                .try_fold(spot, |spot, c| self.render_cell(&pen, spot, c, &mut canvas));

            // Render remaining cells if end of buffer reached before end of display.
            if let Some(spot) = rest {
                self.render_rest(&pen, spot, &mut canvas);
            }

            canvas.draw();
        }
    }
}

/// This module implements the [scrolling](Rendering::Scrolling) engine.
mod scrolling {
    use super::*;
    use crate::buffer::Buffer;
    use crate::canvas::Canvas;
    use crate::nav;
    use crate::style::{Pen, Styler};
    use crate::window::Window;
    use std::cell::Ref;
    use std::cmp;

    pub struct Engine {
        /// Global configuration.
        config: ConfigurationRef,

        /// Underlying buffer used for navigation and rendering.
        buffer: BufferRef,

        /// Buffer position that corresponds to the cursor position.
        pos: usize,

        /// Styler used to create pens for rendering.
        styler: Styler,

        /// Window for rendering contents of the buffer.
        window: WindowRef,

        /// Number of rows for displaying text.
        rows: u32,

        /// Number of columns for displaying text.
        cols: u32,

        /// Indicates when the left margin is enabled.
        margin_enabled: bool,

        /// Top row of the display.
        top_row: Row,

        /// Cursor row of the display.
        cur_row: Row,

        /// An offset from the beginning of any given line in the buffer that is used to
        /// compute the character of the left-most column of the display.
        ///
        /// In effect, the offset increases as the display scrolls left and decreases when
        /// scrolling right.
        offset: usize,

        /// An optional offset and column to which the cursor should _snap_ when moving up
        /// and down.
        snap: Option<(usize, u32)>,

        /// Position of the cursor on the display.
        cursor: Point,
    }

    /// A _row_ on the display that corresponds to a _line_ in the buffer.
    #[derive(Copy, Clone)]
    struct Row {
        /// Buffer position corresponding to the first character of the buffer line.
        line_pos: usize,

        /// Length of the buffer line, including the `\n` if one exists.
        line_len: usize,

        /// The `0`-based number of the buffer line.
        line: u32,

        /// Indicates that the buffer line is the bottom-most line in the buffer.
        is_bottom: bool,
    }

    // Represents a point on the display and additional state used during the rendering
    // process.
    struct Spot<'a> {
        /// An offset representing the leftmost character on any given row.
        ///
        /// This value does not change during the rendering process.
        offset: usize,

        /// Current rendering position in the buffer.
        pos: usize,

        /// A position in the buffer representing the leftmost column on the display.
        start_pos: usize,

        /// Current rendering row.
        row: u32,

        /// Current rendering column.
        col: u32,

        /// Current rendering line number, which is `1`-based.
        line: u32,

        /// Reference to the tokenizer used for syntax coloring.
        ///
        /// This value does not change during the rendering process.
        tok: &'a Tokenizer,

        /// Current rendering syntax cursor.
        syn: Cursor,
    }

    impl Row {
        /// Returns possibly different values of `offset` and `col` if `col` extends
        /// beyond the end of the row or the combination of both would result in a column
        /// that extends beyond either edge of the display, the width of which is specified
        /// in `cols`.
        fn snap(&self, offset: usize, col: u32, cols: u32) -> (usize, u32) {
            // Calculate length of line exclusive of `\n`. Note that all rows except
            // the bottom row have line lengths > 0 because such rows always minimally
            // contain `\n`.
            let len = if self.is_bottom {
                self.line_len
            } else {
                self.line_len - 1
            };

            // Calculate offset of `col` from beginning of buffer line, but do not allow
            // value to extend beyond end of line.
            let ofs = cmp::min(offset + (col as usize), len);
            if ofs < offset {
                // Snapped column is left of left margin, which can only happen if
                // line offset of this row is > 0. Adjust line offset such that snapped
                // column aligns to leftmost edge.
                (len, 0)
            } else if ofs < offset + (cols as usize) {
                // Snapped column is visible on current display, so keep the current line
                // offset to avoid jitter, and modify the snapped column accordingly.
                (offset, (ofs - offset) as u32)
            } else {
                // Snapped column is right of right margin, so adjust line offset and
                // snapped column to align to rightmost edge.
                (ofs - (cols as usize) + 1, cols - 1)
            }
        }

        /// Returns the buffer position of `col` relative to the starting position of
        /// the row and the specified display `offset`.
        #[inline]
        fn pos_of(&self, offset: usize, col: u32) -> usize {
            self.line_pos + offset + (col as usize)
        }

        /// Returns the offset and column number of `pos` relative to the starting position
        /// of this row and the specified `offset`.
        fn col_of(&self, offset: usize, pos: usize, cols: u32) -> (usize, u32) {
            // Adjust line offset if given `pos` would result in a column number left of
            // left margin.
            let offset = cmp::min(pos, self.line_pos + offset) - self.line_pos;
            let col = (pos - (self.line_pos + offset)) as u32;

            // Snap computed column since it may extend beyond rightmost edge,
            self.snap(offset, col, cols)
        }

        /// Returns the buffer position of the right-most column number of this row.
        #[inline]
        fn end_col_pos(&self) -> usize {
            self.line_pos
                + if self.is_bottom {
                    self.line_len
                } else {
                    self.line_len - 1
                }
        }

        /// Returns the buffer position at the end of the row.
        #[inline]
        fn end_pos(&self) -> usize {
            self.line_pos + self.line_len
        }

        /// Moves _up_ by `try_rows` relative to this row, or possibly fewer rows if the
        /// top of the buffer is reached, returning a pair containing the resulting row
        /// and the actual number of rows moved.
        fn up(&self, buffer: &Buffer, try_rows: u32) -> (Row, u32) {
            let mut row = *self;
            for rows in 0..try_rows {
                if let Some(r) = row.prev(buffer) {
                    row = r;
                } else {
                    return (row, rows);
                }
            }
            (row, try_rows)
        }

        /// Moves _down_ by `try_rows` relative to this row, or possibly fewer rows if the
        /// bottom of the buffer is reached, returning a pair containing the resulting row
        /// and the actual number of rows moved.
        fn down(&self, buffer: &Buffer, try_rows: u32) -> (Row, u32) {
            let mut row = *self;
            for rows in 0..try_rows {
                if let Some(r) = row.next(buffer) {
                    row = r
                } else {
                    return (row, rows);
                }
            }
            (row, try_rows)
        }

        /// Finds the row _up_ from this row that contains `pos`, returning a pair
        /// containing the resulting row and the total number of rows moved.
        fn find_up(&self, buffer: &Buffer, pos: usize) -> (Row, u32) {
            let mut row = *self;
            let mut rows = 0;
            while pos < row.line_pos {
                row = row.prev_unchecked(buffer);
                rows += 1;
            }
            (row, rows)
        }

        /// Finds the row _down_ from this row that contains `pos`, returning a pair
        /// containing the resulting row and the total number of rows moved.
        fn find_down(&self, buffer: &Buffer, pos: usize) -> (Row, u32) {
            let mut row = *self;
            let mut rows = 0;
            while pos >= row.end_pos() && !row.is_bottom {
                row = row.next_unchecked(buffer);
                rows += 1;
            }
            (row, rows)
        }

        /// Returns an update version of this row based on the assumption of underlying
        /// changes to the buffer.
        ///
        /// Note that neither `line_pos` nor `line` are modified as part of this update,
        /// as those are assumed to be unchanged.
        ///
        /// The rationale for this function is that an insertion or deletion of text is
        /// always relative to the current row, and that such a change would never
        /// alter the values noted above.
        fn update(&self, buffer: &Buffer) -> Row {
            let (next_pos, is_bottom) = nav::find_next_line(buffer, self.line_pos);
            let line_len = next_pos - self.line_pos;
            Row {
                line_len,
                is_bottom,
                ..*self
            }
        }

        /// Returns the row preceding this row, or `None` if this row is already at the
        /// top of the buffer.
        fn prev(&self, buffer: &Buffer) -> Option<Row> {
            if self.line_pos == 0 {
                None
            } else {
                let pos = self.line_pos - 1;
                let (line_pos, next_pos, is_bottom) = Self::find_row_bounds(buffer, pos);
                let line_len = next_pos - line_pos;
                let r = Row {
                    line_pos,
                    line_len,
                    line: self.line - 1,
                    is_bottom,
                };
                Some(r)
            }
        }

        /// An unchecked version of [`prev`](Self::prev) that assumes this row is not at the
        /// top of the buffer.
        fn prev_unchecked(&self, buffer: &Buffer) -> Row {
            self.prev(buffer)
                .unwrap_or_else(|| panic!("row already at top of buffer"))
        }

        /// Returns the row following this row, or `None` if this row is already at the
        /// bottom of the buffer.
        fn next(&self, buffer: &Buffer) -> Option<Row> {
            if self.is_bottom {
                None
            } else {
                let line_pos = self.line_pos + self.line_len;
                let (next_pos, is_bottom) = nav::find_next_line(buffer, line_pos);
                let line_len = next_pos - line_pos;
                let r = Row {
                    line_pos,
                    line_len,
                    line: self.line + 1,
                    is_bottom,
                };
                Some(r)
            }
        }

        /// An unchecked version of [`next`](Self::next) that assumes this row is not at the
        /// top of the buffer.
        fn next_unchecked(&self, buffer: &Buffer) -> Row {
            self.next(buffer)
                .unwrap_or_else(|| panic!("row already at bottom of buffer"))
        }

        /// Finds and returns the row that contains `pos`.
        ///
        /// This function is expensive because it must calculate the line number corresponding
        /// to `pos` by performing a linear scan from the beginning of the buffer.
        fn find_row(buffer: &Buffer, pos: usize) -> Row {
            let (line_pos, next_pos, is_bottom) = Self::find_row_bounds(buffer, pos);
            let line_len = next_pos - line_pos;
            Row {
                line_pos,
                line_len,
                line: nav::find_location(buffer, line_pos).line,
                is_bottom,
            }
        }

        /// Returns a tuple, relative to the line in `buffer` corresponding to `pos`, containing
        /// the position of the first character on that line, the position of the first
        /// character of the next line, and a boolean value indicating if the end of buffer
        /// has been reached.
        fn find_row_bounds(buffer: &Buffer, pos: usize) -> (usize, usize, bool) {
            let line_pos = nav::find_start_line(buffer, pos);
            let (next_pos, is_bottom) = nav::find_next_line(buffer, pos);
            (line_pos, next_pos, is_bottom)
        }
    }

    #[allow(clippy::derivable_impls, reason = "retain expressiveness")]
    impl Default for Row {
        fn default() -> Row {
            Row {
                line_pos: 0,
                line_len: 0,
                line: 0,
                is_bottom: false,
            }
        }
    }

    impl<'a> Spot<'a> {
        /// Creates a spot representing the top-left position on the display.
        fn new(engine: &Engine, tok: &'a Tokenizer, syn: Cursor) -> Spot<'a> {
            let offset = engine.offset;
            let pos = engine.top_row.line_pos;
            Spot {
                offset,
                pos,
                start_pos: pos + offset,
                row: 0,
                col: 0,
                line: engine.top_row.line + 1,
                tok,
                syn,
            }
        }

        fn next(mut self) -> Spot<'a> {
            self.pos += 1;
            self.col = if self.pos > self.start_pos {
                self.col + 1
            } else {
                0
            };
            self.syn = self.tok.forward(self.syn, 1);
            self
        }

        fn next_line(mut self) -> Spot<'a> {
            self.pos += 1;
            self.start_pos = self.pos + self.offset;
            self.row += 1;
            self.col = 0;
            self.line += 1;
            self.syn = self.tok.forward(self.syn, 1);
            self
        }
    }

    impl Engine {
        /// Creates a new rendering engine with an unattached window using `config`
        /// and `buffer`.
        pub fn new(config: ConfigurationRef, buffer: BufferRef) -> Engine {
            let pos = buffer.borrow().get_pos();
            let styler = Styler::new(config.clone());

            Engine {
                config,
                buffer,
                pos,
                styler,
                window: Window::zombie().into_ref(),
                rows: 0,
                cols: 0,
                margin_enabled: false,
                top_row: Row::default(),
                cur_row: Row::default(),
                offset: 0,
                snap: None,
                cursor: Point::ORIGIN,
            }
        }

        /// Renders an individual cell for the character `c`, returning the next rendering
        /// context or `None` if rendering has finished.
        fn render_cell<'a>(
            &self,
            pen: &Pen,
            spot: Spot<'a>,
            c: char,
            canvas: &mut Canvas,
        ) -> Option<Spot<'a>> {
            self.render_margin(pen, &spot, canvas);
            let (row, col) = (spot.row, spot.col + self.margin_cols());
            let color = spot.syn.color();
            let spot = if c == '\n' {
                if spot.pos < spot.start_pos {
                    // Character is left of left margin, hnece not visible, so just
                    // fill entire line with blanks.
                    canvas.fill_cell_from(row, col, pen.as_text(' ', spot.pos, row, color));
                } else if spot.col < self.cols {
                    // Character is visible on display, so render it and fill remaining
                    // line with blanks.
                    canvas.set_cell(row, col, pen.as_text(c, spot.pos, row, color));
                    canvas.fill_cell_from(row, col + 1, pen.as_text(' ', spot.pos, row, color));
                }
                spot.next_line()
            } else {
                if spot.pos >= spot.start_pos && spot.col < self.cols {
                    // Character is visible on display, so render it and fill remaining
                    // line with blanks.
                    canvas.set_cell(row, col, pen.as_text(c, spot.pos, row, color));
                }
                spot.next()
            };
            if spot.row < self.rows {
                Some(spot)
            } else {
                None
            }
        }

        /// Renders the remainder of the displayable area which is considered empty space.
        ///
        /// This function gets invoked when the end of buffer is reached before the entire
        /// canvas is rendered.
        fn render_rest(&self, pen: &Pen, spot: Spot, canvas: &mut Canvas) {
            self.render_margin(pen, &spot, canvas);

            // Blank out rest of existing row.
            let (row, col) = (spot.row, spot.col + self.margin_cols());
            let color = spot.syn.color();
            canvas.fill_cell_from(row, col, pen.as_text(' ', spot.pos, spot.row, color));

            // Blank out remaining rows.
            for row in (spot.row + 1)..self.rows {
                if self.margin_enabled {
                    canvas.fill_cell(row, 0..MARGIN_COLS, pen.as_margin(' '));
                }
                canvas.fill_cell_from(row, MARGIN_COLS, pen.as_blank());
            }
        }

        /// Renders the margin if line numbering is enabled and the rendering context is
        /// on the first column of any row.
        fn render_margin(&self, pen: &Pen, spot: &Spot, canvas: &mut Canvas) {
            if spot.col == 0 && self.margin_enabled {
                if spot.line < LINE_LIMIT {
                    let s = format!("{:>cols$} ", spot.line, cols = MARGIN_COLS as usize - 1);
                    for (col, c) in s.char_indices() {
                        canvas.set_cell(spot.row, col as u32, pen.as_line(c, spot.line));
                    }
                } else {
                    canvas.fill_cell(spot.row, 0..CLIP_UPPER_COLS, pen.as_line('-', spot.line));
                    let s = format!(
                        "{:0>cols$}",
                        spot.line % 10_u32.pow(CLIP_LOWER_COLS),
                        cols = CLIP_LOWER_COLS as usize,
                    );
                    for (col, c) in s.char_indices() {
                        canvas.set_cell(
                            spot.row,
                            col as u32 + CLIP_UPPER_COLS,
                            pen.as_line(c, spot.line),
                        );
                    }
                    canvas.set_cell(spot.row, MARGIN_COLS, pen.as_margin(' '));
                }
            }
        }

        /// Moves `cur_row` _up_ by `try_rows` or possibly fewer if the top of the buffer
        /// is reached, returning the actual number of rows moved.
        fn up_cur_row(&mut self, try_rows: u32) -> u32 {
            let (cur_row, rows) = self.cur_row.up(&self.buffer(), try_rows);
            self.cur_row = cur_row;
            rows
        }

        /// Moves `cur_row` _down_ by `try_rows` or possibly fewer if the bottom of the
        /// buffer is reached, returning the actual number of rows moved.
        fn down_cur_row(&mut self, try_rows: u32) -> u32 {
            let (cur_row, rows) = self.cur_row.down(&self.buffer(), try_rows);
            self.cur_row = cur_row;
            rows
        }

        /// Moves `top_row` _up_ by `try_rows` or possibly fewer if the top of the buffer
        /// is reached, returning the actual number of rows moved.
        fn up_top_row(&mut self, try_rows: u32) -> u32 {
            let (top_row, rows) = self.top_row.up(&self.buffer(), try_rows);
            self.top_row = top_row;
            rows
        }

        /// Moves `top_row` _down_ by `try_rows` or possibly fewer if the bottom of the
        /// buffer is reached, returning the actual number of rows moved.
        fn down_top_row(&mut self, try_rows: u32) -> u32 {
            let (top_row, rows) = self.top_row.down(&self.buffer(), try_rows);
            self.top_row = top_row;
            rows
        }

        /// Sets `top_row` to `try_rows` before `cur_row` or possibly fewer if the top of
        /// the buffer is reached, returning the actual number of rows moved.
        fn set_top_row(&mut self, try_rows: u32) -> u32 {
            let (top_row, rows) = self.cur_row.up(&self.buffer(), try_rows);
            self.top_row = top_row;
            rows
        }

        /// Updates `cur_row` based on the assumption of underlying changes to the buffer.
        fn update_cur_row(&mut self) {
            let cur_row = self.cur_row.update(&self.buffer());
            self.cur_row = cur_row;
        }

        /// Updates `top_row` based on the assumption of underlying changes to the buffer.
        fn update_top_row(&mut self) {
            let top_row = self.top_row.update(&self.buffer());
            self.top_row = top_row;
        }

        /// Moves `cur_row` _up_ until the resulting line contains `pos`, returning the
        /// total number of rows moved.
        fn find_up_cur_row(&mut self, pos: usize) -> u32 {
            let (cur_row, rows) = self.cur_row.find_up(&self.buffer(), pos);
            self.cur_row = cur_row;
            rows
        }

        /// Moves `cur_row` _down_ until the resulting line contains `pos`, returning the
        /// total number of rows moved.
        fn find_down_cur_row(&mut self, pos: usize) -> u32 {
            let (cur_row, rows) = self.cur_row.find_down(&self.buffer(), pos);
            self.cur_row = cur_row;
            rows
        }

        #[inline]
        fn margin_cols(&self) -> u32 {
            if self.margin_enabled { MARGIN_COLS } else { 0 }
        }

        #[inline]
        fn buffer(&self) -> Ref<'_, Buffer> {
            self.buffer.borrow()
        }
    }

    impl Renderer for Engine {
        fn kind(&self) -> Rendering {
            Rendering::Scrolling
        }

        fn attach(&mut self, window: WindowRef) {
            // Allocate leftmost columns of window to line numbers, but only if enabled and
            // total width of window is large enough to reasonably accommodate.
            self.window = window;
            let Size { rows, cols } = self.window.borrow().canvas.borrow().size();
            self.margin_enabled = self.config.settings.lines && cols >= MARGIN_COLS * 2;
            self.rows = rows;
            self.cols = cols - self.margin_cols();
        }

        fn detach(&mut self) {
            self.attach(Window::zombie().into_ref());
        }

        fn is_attached(&self) -> bool {
            !self.window.borrow().is_zombie()
        }

        #[inline(always)]
        fn rows(&self) -> u32 {
            self.rows
        }

        fn size(&self) -> Size {
            Size::new(self.rows, self.cols)
        }

        #[inline(always)]
        fn pos(&self) -> usize {
            self.pos
        }

        #[inline(always)]
        fn cursor(&self) -> Point {
            self.cursor
        }

        #[inline(always)]
        fn location(&self) -> Location {
            Location::new(self.cur_row.line, (self.offset as u32) + self.cursor.col)
        }

        #[inline]
        fn line(&self) -> (usize, usize) {
            (
                self.cur_row.line_pos,
                self.cur_row.line_pos + self.cur_row.line_len,
            )
        }

        #[inline]
        fn top(&self) -> (usize, usize) {
            (
                self.top_row.line_pos,
                self.top_row.line_pos + self.cur_row.line_len,
            )
        }

        fn focus_cursor(&mut self, cursor: Point) {
            // Ensure target cursor is bounded by effective area of canvas, which takes
            // into account left margin if enabled.
            let try_row = cmp::min(cursor.row, self.rows);
            let try_col = if cursor.col < self.margin_cols() {
                0
            } else {
                cmp::min(cursor.col - self.margin_cols(), self.cols)
            };

            // Find effective cursor location and buffer position by moving down from
            // top row of display.
            let (cur_row, row) = self.top_row.down(&self.buffer(), try_row);
            self.cur_row = cur_row;
            let (offset, col) = self.cur_row.snap(self.offset, try_col, self.cols);
            self.offset = offset;
            self.pos = self.cur_row.pos_of(self.offset, col);
            self.snap = Some((self.offset, col));
            self.cursor = Point::new(row, col);
        }

        fn align_cursor(&mut self, align: Align) {
            // Determine ideal row where cursor would like to be focused, though this should
            // be considered a hint.
            let try_row = match align {
                Align::Auto => cmp::min(self.cursor.row, self.rows - 1),
                Align::Center => self.rows / 2,
                Align::Top => 0,
                Align::Bottom => self.rows - 1,
                Align::Row(row) => cmp::min(row, self.rows - 1),
            };

            // Tries to position cursor on target row, but no guarantee depending on proximity
            // of row to top of buffer.
            let cur_row = Row::find_row(&self.buffer(), self.pos);
            self.cur_row = cur_row;
            let (top_row, row) = self.cur_row.up(&self.buffer(), try_row);
            self.top_row = top_row;
            let (offset, col) = self.cur_row.col_of(self.offset, self.pos, self.cols);
            self.snap = None;
            self.offset = offset;
            self.cursor = Point::new(row, col);
        }

        fn show_cursor(&mut self) {
            let cursor = if self.margin_enabled {
                self.cursor + Size::cols(MARGIN_COLS)
            } else {
                self.cursor
            };
            self.window.borrow().canvas.borrow_mut().set_cursor(cursor);
        }

        fn move_up(&mut self, try_rows: u32, pin: bool) -> u32 {
            let rows = self.up_cur_row(try_rows);
            if rows > 0 {
                let row = if pin {
                    if rows < try_rows {
                        // Cursor reached top of buffer before advancing by desired number of
                        // rows, so resulting row is always top of display.
                        self.set_top_row(0)
                    } else {
                        // Try finding new top line by stepping backwards by number of rows
                        // equivalent to current row of cursor.
                        self.set_top_row(self.cursor.row)
                    }
                } else if rows > self.cursor.row {
                    // Cursor would have moved beyond top of display.
                    self.set_top_row(0)
                } else {
                    // Cursor remains visible without changing top line.
                    self.cursor.row - rows
                };
                let (try_offset, try_col) =
                    self.snap.take().unwrap_or((self.offset, self.cursor.col));
                let (offset, col) = self.cur_row.snap(try_offset, try_col, self.cols);
                self.offset = offset;
                self.pos = self.cur_row.pos_of(self.offset, col);
                self.snap = Some((try_offset, try_col));
                self.cursor = Point::new(row, col);
            }
            rows
        }

        fn move_down(&mut self, try_rows: u32, pin: bool) -> u32 {
            let rows = self.down_cur_row(try_rows);
            if rows > 0 {
                let row = if pin {
                    // Keeping cursor on current row is guaranteed, because top line can
                    // always move down without reaching bottom of buffer.
                    let _ = self.down_top_row(rows);
                    self.cursor.row
                } else if self.cursor.row + rows < self.rows {
                    // Cursor remains visible without changing top line.
                    self.cursor.row + rows
                } else {
                    // Cursor would have moved beyond bottom of display.
                    self.set_top_row(self.rows - 1)
                };
                let (try_offset, try_col) =
                    self.snap.take().unwrap_or((self.offset, self.cursor.col));
                let (offset, col) = self.cur_row.snap(try_offset, try_col, self.cols);
                self.offset = offset;
                self.pos = self.cur_row.pos_of(self.offset, col);
                self.snap = Some((try_offset, try_col));
                self.cursor = Point::new(row, col);
            }
            rows
        }

        fn move_start(&mut self) {
            self.move_to(self.cur_row.line_pos, Align::Row(self.cursor.row));
        }

        fn move_end(&mut self) {
            self.move_to(self.cur_row.end_col_pos(), Align::Row(self.cursor.row));
        }

        fn move_to(&mut self, pos: usize, align: Align) {
            let row = if pos < self.top_row.line_pos {
                let _ = self.find_up_cur_row(pos);
                let rows = match align {
                    Align::Top | Align::Auto => 0,
                    Align::Center => self.rows / 2,
                    Align::Bottom => self.rows - 1,
                    Align::Row(row) => cmp::min(row, self.rows - 1),
                };
                self.set_top_row(rows)
            } else if pos < self.cur_row.line_pos {
                let row = self.cursor.row - self.find_up_cur_row(pos);
                let maybe_rows = match align {
                    Align::Auto => None,
                    Align::Top => Some(0),
                    Align::Center => Some(self.rows / 2),
                    Align::Bottom => Some(self.rows - 1),
                    Align::Row(row) => Some(cmp::min(row, self.rows - 1)),
                };
                if let Some(rows) = maybe_rows {
                    self.set_top_row(rows)
                } else {
                    row
                }
            } else if pos < self.cur_row.end_pos() {
                let maybe_rows = match align {
                    Align::Auto => None,
                    Align::Top => Some(0),
                    Align::Center => Some(self.rows / 2),
                    Align::Bottom => Some(self.rows - 1),
                    Align::Row(row) => Some(cmp::min(row, self.rows - 1)),
                };
                if let Some(rows) = maybe_rows {
                    self.set_top_row(rows)
                } else {
                    self.cursor.row
                }
            } else {
                let rows = self.find_down_cur_row(pos);
                let row = match align {
                    Align::Auto => cmp::min(self.cursor.row + rows, self.rows - 1),
                    Align::Top => 0,
                    Align::Center => self.rows / 2,
                    Align::Bottom => self.rows - 1,
                    Align::Row(row) => cmp::min(row, self.rows - 1),
                };
                self.set_top_row(row)
            };
            self.pos = pos;
            let (offset, col) = self.cur_row.col_of(self.offset, self.pos, self.cols);
            self.offset = offset;
            self.snap = None;
            self.cursor = Point::new(row, col);
        }

        fn scroll_up(&mut self, try_rows: u32) -> u32 {
            let rows = self.down_top_row(try_rows);
            if rows > 0 {
                let (row, col) = if rows > self.cursor.row {
                    // Cursor would have moved beyond top of display, which means current
                    // buffer position changes accordingly.
                    self.cur_row = self.top_row;
                    let (try_offset, try_col) =
                        self.snap.take().unwrap_or((self.offset, self.cursor.col));
                    self.snap = Some((try_offset, try_col));
                    let (offset, col) = self.cur_row.snap(try_offset, try_col, self.cols);
                    self.offset = offset;
                    self.pos = self.cur_row.pos_of(self.offset, col);
                    (0, col)
                } else {
                    // Cursor still visible on display.
                    (self.cursor.row - rows, self.cursor.col)
                };
                self.cursor = Point::new(row, col);
            }
            rows
        }

        fn scroll_down(&mut self, try_rows: u32) -> u32 {
            let rows = self.up_top_row(try_rows);
            if rows > 0 {
                let row = self.cursor.row + rows;
                let (row, col) = if row < self.rows {
                    // Cursor still visible on display.
                    (row, self.cursor.col)
                } else {
                    // Cursor would have moved beyond bottom of display, which means current
                    // buffer position changes accordingly.
                    let _ = self.up_cur_row(row - self.rows + 1);
                    let (try_offset, try_col) =
                        self.snap.take().unwrap_or((self.offset, self.cursor.col));
                    self.snap = Some((try_offset, try_col));
                    let (offset, col) = self.cur_row.snap(try_offset, try_col, self.cols);
                    self.offset = offset;
                    self.pos = self.cur_row.pos_of(self.offset, col);
                    (self.rows - 1, col)
                };
                self.cursor = Point::new(row, col);
            }
            rows
        }

        fn insert(&mut self, len: usize) {
            if len > 0 {
                // Buffer position moves forward.
                self.pos += len;

                // Update current row since insertion will changed boundaries for future
                // navigation.
                self.update_cur_row();

                // Find possibly new current row.
                let row = self.cursor.row + self.find_down_cur_row(self.pos);

                // New current row could extend beyond display boundary, so update or
                // find new top row.
                let row = if row < self.rows {
                    self.update_top_row();
                    row
                } else {
                    self.set_top_row(self.rows - 1)
                };
                let (offset, col) = self.cur_row.col_of(self.offset, self.pos, self.cols);
                self.offset = offset;
                self.snap = None;
                self.cursor = Point::new(row, col);
            }
        }

        fn remove(&mut self) {
            // Removal of text requires both current row and top row to be updated since
            // boundaries may have changed.
            self.update_cur_row();
            self.update_top_row();
            let (offset, col) = self.cur_row.col_of(self.offset, self.pos, self.cols);
            self.offset = offset;
            self.snap = None;
            self.cursor.col = col;
        }

        fn render(&mut self, tok: &Tokenizer, syn: Cursor, selected: Option<Range<usize>>) {
            // Create pen to format characters.
            let pen = self
                .styler
                .pen(self.cursor, self.top_row.line + 1, selected);

            // Initialize spot representing top-left cell.
            let spot = Spot::new(self, tok, syn);

            // Borrow canvas once and pass to rendering functions to optimize.
            let window = self.window.borrow();
            let mut canvas = window.canvas.borrow_mut();

            // Render cells until end of display or end of buffer reached.
            let rest = self
                .buffer()
                .forward(spot.pos)
                .try_fold(spot, |spot, c| self.render_cell(&pen, spot, c, &mut canvas));

            // Render remaining cells if end of buffer reached before end of display.
            if let Some(spot) = rest {
                self.render_rest(&pen, spot, &mut canvas);
            }

            canvas.draw();
        }
    }
}
