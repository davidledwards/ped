//! Implements the [wrapping](Rendering::Wrapping) rendering engine.
//!
//! A wrapping engine wraps long lines of text across multiple rows on the display.
//! Text can be scrolled up and and down, but not left and right.
//!
//! This diagram illustrates the manner in which long lines automatically wrap, with
//! numbers on the left depicting the _line_ number and those on the right showing the
//! _row_ number.
//!
//! ```text
//!  line                                 row
//!       +-----------------------------+
//!    0  |fn main() -> ExitCode {      |  0
//!    1  |   match run() {             |  1
//!    2  |      Err(e) => {            |  2
//!    3  |         let _ = writeln!(std|  3
//!       |err(), "{e}");               |  4
//!    4  |         ExitCode::from(1)   |  5
//!    5  |      }                      |  6
//!       +-----------------------------+
//! ```

use super::*;
use crate::buffer::Buffer;
use crate::canvas::Canvas;
use crate::nav;
use crate::style::Pen;
use crate::window::Window;
use std::cell::Ref;
use std::cmp;

pub struct WrappingRenderer {
    /// Global configuration.
    config: ConfigurationRef,

    /// Underlying buffer used for navigation and rendering.
    buffer: BufferRef,

    /// Buffer position that corresponds to the cursor position.
    pos: usize,

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

    /// Position of the cursor on the display when visible, otherwise `None`.
    cursor: Option<Point>,

    /// Location of the cursor in the buffer.
    loc: Location,
}

/// A _row_ on the display that corresponds to a _line_ in the buffer.
///
/// A row should not be confused with a line in the buffer, the latter of which
/// could conceivably span more than one row on the display.
///
/// Consider the following line of text in the buffer, where `↵` is a `\n` character.
/// The line, including `\n`, is `44` characters.
///
/// ```text
/// 00000000001111111111222222222233333333334444
/// 01234567890123456789012345678901234567890123
/// --------------------------------------------
/// The quick brown fox jumps over the lazy dog↵
/// ```
///
/// Now consider that the display width is `16` characters. The line would wrap across
/// multiple rows on the display. The numbers on the left show the _line_ number, whereas
/// those on the right represent the _row_ number.
///
/// ```text
///  line                    row
///       +----------------+
///    0  |The quick brown |  0
///       |fox jumps over t|  1
///       |he lazy dog↲    |  2
///    1  |                |  3
///       +----------------+
/// ```
///
/// This shows how each of the three rows would be initialized.
///
/// ```text
///                       +----------------+
///                       |  row_pos = 16  |
///                       |  row_len = 16  |
///                       |  line_pos = 0  |
///                       |  line_len = 44 |
///                       |  line = 0      |
///                       Row 1 -----------+
///                         |
///                   ____________
///                  /            \
///                 v              v
/// The quick brown fox jumps over the lazy dog↵
/// ^              ^                ^          ^
///  \____________/                  \________/
///         |                             |
///       Row 0 -----------+            Row 2 -----------+
///       |  row_pos = 0   |            |  row_pos = 32  |
///       |  row_len = 16  |            |  row_len = 12  |
///       |  line_pos = 0  |            |  line_pos = 0  |
///       |  line_len = 44 |            |  line_len = 44 |
///       |  line = 0      |            |  line = 0      |
///       +----------------+            +----------------+
/// ```
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
    wrapped: bool,

    /// Reference to the tokenizer used for syntax coloring.
    tokenizer: &'a Tokenizer,

    /// Current token position for syntax coloring.
    token_pos: Position,
}

impl Row {
    /// Finds and returns the row that contains `pos`.
    ///
    /// This function is expensive because it must calculate the line number
    /// corresponding to `pos` by performing a linear scan from the beginning of the
    /// buffer.
    fn find_row(buffer: &Buffer, pos: usize, cols: u32) -> Row {
        let (line_pos, next_pos, is_bottom) = find_line_bounds(buffer, pos);
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
        self.row_pos + self.adjusted_len()
    }

    /// Returns `true` if the row of this line wraps at least to the next row,
    /// indicating that the buffer line is longer than the width of the display.
    #[inline]
    fn is_wrapping(&self) -> bool {
        self.row_pos + self.row_len < self.line_pos + self.line_len
    }

    /// Returns a possibly smaller value of `col` if it extends beyond the end of
    /// the row.
    ///
    /// In most cases, the right-most column aligns to the last character of the row,
    /// which is usually `\n` but may also be any other character if the row wraps.
    /// However, if this is the bottom-most row in the buffer, there is no terminating
    /// `\n`, and thus the right-most column is right of the last character.
    #[inline]
    fn snap(&self, col: u32) -> u32 {
        cmp::min(col, self.adjusted_len() as u32)
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
}

impl Rowable for Row {
    #[inline]
    fn start_pos(&self) -> usize {
        self.row_pos
    }

    #[inline]
    fn end_pos(&self) -> usize {
        self.row_pos + self.row_len
    }

    #[inline]
    fn is_bottom(&self) -> bool {
        self.is_bottom && self.row_len < (self.cols as usize)
    }

    #[inline]
    fn line(&self) -> (usize, usize) {
        (self.line_pos, self.line_pos + self.line_len)
    }

    #[inline]
    fn adjusted_len(&self) -> usize {
        if self.is_bottom() {
            self.row_len
        } else {
            self.row_len - 1
        }
    }

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
            let (line_pos, next_pos, is_bottom) = find_line_bounds(buffer, pos);
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
    fn new(renderer: &WrappingRenderer, tokenizer: &'a Tokenizer, token_pos: Position) -> Spot<'a> {
        Spot {
            pos: renderer.top_row.row_pos,
            row: 0,
            col: 0,
            line: renderer.top_row.line + 1,
            wrapped: false,
            tokenizer,
            token_pos,
        }
    }

    fn next_col(mut self) -> Spot<'a> {
        self.pos += 1;
        self.col += 1;
        self.token_pos = self.tokenizer.forward(self.token_pos, 1);
        self
    }

    fn next_row(mut self) -> Spot<'a> {
        self.pos += 1;
        self.row += 1;
        self.col = 0;
        self.wrapped = true;
        self.token_pos = self.tokenizer.forward(self.token_pos, 1);
        self
    }

    fn next_line(mut self) -> Spot<'a> {
        self.pos += 1;
        self.row += 1;
        self.col = 0;
        self.line += 1;
        self.wrapped = false;
        self.token_pos = self.tokenizer.forward(self.token_pos, 1);
        self
    }
}

impl WrappingRenderer {
    /// Creates a _wrapping_ renderer with an unattached window using `config`
    /// and `buffer`.
    pub fn new(config: ConfigurationRef, buffer: BufferRef) -> WrappingRenderer {
        let pos = buffer.borrow().get_pos();

        WrappingRenderer {
            config,
            buffer,
            pos,
            window: Window::zombie().into_ref(),
            rows: 0,
            cols: 0,
            margin_enabled: false,
            top_row: Row::default(),
            cur_row: Row::default(),
            snap: None,
            cursor: Some(Point::ORIGIN),
            loc: Location::TOP,
        }
    }

    /// Returns the cursor position on the display, but also has the potential side
    /// effect of realigning the cursor if _hidden_.
    fn get_cursor(&mut self) -> Point {
        self.cursor
            .unwrap_or_else(|| self.align_cursor(Align::Center, Justify::Center))
    }

    /// Sets the cursor position to _row_ and _col_, and also updates the location
    /// of the cursor in the buffer.
    fn set_cursor(&mut self, row: u32, col: u32) -> Point {
        let cursor = Point::new(row, col);
        self.cursor = Some(cursor);
        self.loc = Location::new(
            self.cur_row.line,
            (self.cur_row.row_pos - self.cur_row.line_pos) as u32 + col,
        );
        cursor
    }

    /// Possibly sets the cursor position if a previously hidden cursor becomes
    /// visible.
    fn possibly_unhide_cursor(&mut self) {
        // Scrolling down may lead to the current row appearing before top row,
        // which means the cursor can never be visible, so only consider cases
        // where the current row appears after top row.
        if self.top_row.row_pos <= self.cur_row.row_pos {
            // Limit extent of row search by the numbers of rows on display.
            let row_info = self
                .top_row
                .findn_down(&self.buffer(), self.pos, self.rows - 1);

            if let Some((cur_row, row)) = row_info {
                let col = cur_row.col_of(self.pos);
                self.set_cursor(row, col);
            }
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

        // Bind these values locally to improve readability.
        let (row, col) = (spot.row, spot.col + self.margin_cols());
        let color = spot.token_pos.color();

        // Render character.
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
        let color = spot.token_pos.color();
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
            if spot.wrapped {
                canvas.fill_cell(spot.row, 0..MARGIN_COLS, pen.as_margin(' '));
            } else if spot.line < LINE_LIMIT {
                let s = format_line(spot.line);
                for (col, c) in s.char_indices() {
                    canvas.set_cell(spot.row, col as u32, pen.as_line(c, spot.line));
                }
            } else {
                canvas.fill_cell(spot.row, 0..CLIP_UPPER_COLS, pen.as_line('-', spot.line));
                let s = format_line(spot.line);
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

impl Renderer for WrappingRenderer {
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

    #[inline]
    fn rows(&self) -> u32 {
        self.rows
    }

    #[inline]
    fn size(&self) -> Size {
        Size::new(self.rows, self.cols)
    }

    #[inline]
    fn pos(&self) -> usize {
        self.pos
    }

    #[inline]
    fn cursor(&self) -> Option<Point> {
        self.cursor
    }

    #[inline]
    fn location(&self) -> Location {
        self.loc
    }

    #[inline]
    fn line(&self) -> (usize, usize) {
        self.cur_row.line()
    }

    #[inline]
    fn origin(&self) -> usize {
        self.top_row.row_pos
    }

    fn focus_cursor(&mut self, cursor: Point) -> Point {
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
        self.set_cursor(row, col)
    }

    fn align_cursor(&mut self, align: Align, _: Justify) -> Point {
        // Determine ideal row where cursor would like to be focused, though this
        // should be considered a hint.
        let try_row = match align {
            Align::Auto => self
                .cursor
                .map(|cursor| cmp::min(cursor.row, self.rows - 1))
                .unwrap_or(self.rows / 2),
            Align::Center => self.rows / 2,
            Align::Top => 0,
            Align::Bottom => self.rows - 1,
            Align::Row(row) => cmp::min(row, self.rows - 1),
        };

        // Tries to position cursor on target row, but no guarantee depending on
        // proximity of row to top of buffer.
        let cur_row = Row::find_row(&self.buffer(), self.pos, self.cols);
        self.cur_row = cur_row;
        let (top_row, row) = self.cur_row.up(&self.buffer(), try_row);
        self.top_row = top_row;
        let col = self.cur_row.col_of(self.pos);
        self.snap = None;
        self.set_cursor(row, col)
    }

    fn show_cursor(&mut self) {
        match self.cursor {
            Some(cursor) => {
                let cursor = if self.margin_enabled {
                    cursor + Size::cols(MARGIN_COLS)
                } else {
                    cursor
                };
                self.window.borrow().canvas.borrow_mut().set_cursor(cursor);
            }
            None => {
                self.window.borrow().canvas.borrow_mut().hide_cursor();
            }
        }
    }

    fn move_up(&mut self, try_rows: u32, pin: bool) -> u32 {
        let cursor = self.get_cursor();
        let rows = self.up_cur_row(try_rows);
        if rows > 0 {
            let row = if pin {
                if rows < try_rows {
                    // Cursor reached top of buffer before advancing by desired number
                    // of rows, so resulting row is always top of display.
                    self.set_top_row(0)
                } else {
                    // Try finding new top line by stepping backwards by number of rows
                    // equivalent to current row of cursor.
                    self.set_top_row(cursor.row)
                }
            } else if rows > cursor.row {
                // Cursor would have moved beyond top of display.
                self.set_top_row(0)
            } else {
                // Cursor remains visible without changing top line.
                cursor.row - rows
            };
            let try_col = self.snap.take().unwrap_or(cursor.col);
            let col = self.cur_row.snap(try_col);
            self.pos = self.cur_row.pos_of(col);
            self.snap = Some(try_col);
            self.set_cursor(row, col);
        }
        rows
    }

    fn move_down(&mut self, try_rows: u32, pin: bool) -> u32 {
        let cursor = self.get_cursor();
        let rows = self.down_cur_row(try_rows);
        if rows > 0 {
            let row = if pin {
                // Keeping cursor on current row is guaranteed, because top line can
                // always move down without reaching bottom of buffer.
                let _ = self.down_top_row(rows);
                cursor.row
            } else if cursor.row + rows < self.rows {
                // Cursor remains visible without changing top line.
                cursor.row + rows
            } else {
                // Cursor would have moved beyond bottom of display.
                self.set_top_row(self.rows - 1)
            };
            let try_col = self.snap.take().unwrap_or(cursor.col);
            let col = self.cur_row.snap(try_col);
            self.pos = self.cur_row.pos_of(col);
            self.snap = Some(try_col);
            self.set_cursor(row, col);
        }
        rows
    }

    fn move_start(&mut self) {
        let cursor = self.get_cursor();
        self.move_to(self.cur_row.row_pos, Align::Row(cursor.row), Justify::Auto);
    }

    fn move_end(&mut self) {
        let cursor = self.get_cursor();
        self.move_to(
            self.cur_row.end_col_pos(),
            Align::Row(cursor.row),
            Justify::Auto,
        );
    }

    fn move_to(&mut self, pos: usize, align: Align, _: Justify) {
        let cursor = self.get_cursor();
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
            let row = cursor.row - self.find_up_cur_row(pos);
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
                cursor.row
            }
        } else {
            let rows = self.find_down_cur_row(pos);
            let row = match align {
                Align::Auto => cmp::min(cursor.row + rows, self.rows - 1),
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
        self.set_cursor(row, col);
    }

    fn scroll_up(&mut self, try_rows: u32) -> u32 {
        let rows = self.up_top_row(try_rows);
        if rows > 0 {
            if let Some(cursor) = self.cursor {
                let row = cursor.row + rows;
                if row < self.rows {
                    // Cursor still visible on display.
                    self.set_cursor(row, cursor.col);
                } else {
                    // Cursor has moved below bottom of display.
                    self.cursor = None;
                }
            } else {
                // Cursor is hidden but may become visible.
                self.possibly_unhide_cursor();
            }
        }
        rows
    }

    fn scroll_down(&mut self, try_rows: u32) -> u32 {
        let rows = self.down_top_row(try_rows);
        if rows > 0 {
            if let Some(cursor) = self.cursor {
                if rows > cursor.row {
                    // Cursor has moved above top of display.
                    self.cursor = None;
                } else {
                    // Cursor still visible on display.
                    self.set_cursor(cursor.row - rows, cursor.col);
                }
            } else {
                // Cursor is hidden but may become visible.
                self.possibly_unhide_cursor();
            }
        }
        rows
    }

    fn scroll_left(&mut self, _: u32) -> u32 {
        // Action not applicable for wrapping engine.
        0
    }

    fn scroll_right(&mut self, _: u32) -> u32 {
        // Action not applicable for wrapping engine.
        0
    }

    fn insert(&mut self, len: usize) {
        if len > 0 {
            // Buffer position moves forward.
            self.pos += len;

            // Update current row since insertion will changed boundaries for future
            // navigation.
            let cursor = self.get_cursor();
            self.update_cur_row();

            // Find possibly new current row.
            let row = cursor.row + self.find_down_cur_row(self.pos);

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
            self.set_cursor(row, col);
        }
    }

    fn remove(&mut self) {
        // Removal of text requires both current row and top row to be updated since
        // boundaries may have changed.
        let cursor = self.get_cursor();
        self.update_cur_row();
        self.update_top_row();
        let col = self.cur_row.col_of(self.pos);
        self.snap = None;
        self.set_cursor(cursor.row, col);
    }

    fn render(&mut self, tokenizer: &Tokenizer, token_pos: Position, style: &Style) {
        // Create pen to format characters.
        let pen = style.pen(self.cursor, self.cur_row.line + 1);

        // Initialize spot representing top-left cell.
        let spot = Spot::new(self, tokenizer, token_pos);

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
