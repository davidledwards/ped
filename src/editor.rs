//! Editor.
use crate::buffer::{Buffer, BufferRef};
use crate::canvas::CanvasRef;
use crate::color::Color;
use crate::grid::Cell;
use crate::size::{Point, Size};
use crate::theme::ThemeRef;
use crate::window::{BannerRef, Window, WindowRef};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

/// An editing controller with an underlying [Buffer] and an attachable
/// [Window].
pub struct Editor {
    /// An optional path if the buffer is associated with a file.
    path: Option<PathBuf>,

    /// Buffer containing the contents of this editor.
    buffer: BufferRef,

    /// Buffer position corresponding to the cursor.
    cur_pos: usize,

    /// Line on the display representing the top row.
    top_line: Line,

    /// Line on the display representing the cursor row.
    cur_line: Line,

    /// An optional column to which the cursor should *snap* when moving up and down.
    snap_col: Option<u32>,

    /// Position of the cursor in the window.
    cursor: Point,

    /// An optional mark used when selecting text.
    mark: Option<Mark>,

    /// Pane that controls output to the attached window.
    pane: Pane,
}

pub type EditorRef = Rc<RefCell<Editor>>;

/// Represents contextual information for a line on the display.
///
/// A *line* in this context should not be confused with the characterization of
/// a line in [Buffer], which could conceivably span more than one line on the
/// display.
#[derive(Clone)]
struct Line {
    /// Buffer position corresponding to the first character of the display line,
    /// which is always greater than or equal to `row_pos`.
    row_pos: usize,

    /// Length of the display line, including the `\n` if one exists.
    row_len: usize,

    /// Buffer position corresponding to the first character of the buffer line,
    /// which is always less than or equal to `row_pos`.
    line_pos: usize,

    /// Length of the buffer line, including the `\n` if one exists.
    line_len: usize,

    /// The `0`-based number of the buffer line.
    line_nbr: usize,

    /// Indicates that the buffer line is the bottom-most line in the buffer.
    line_bottom: bool,
}

/// Controls the rendering of displayable content to a [Window].
struct Pane {
    /// Color theme that applies to the window.
    theme: ThemeRef,

    /// Canvas associated with the window.
    canvas: CanvasRef,

    /// Banner associated with the window.
    banner: BannerRef,

    /// Number of rows in [`View::canvas`].
    rows: u32,

    /// Number of columns in [`View::canvas`].
    cols: u32,
}

/// Context provided to draw functions in [`Pane`].
///
/// By encapsulating theme-dependent decisions and relevant state information in this
/// drawing context, [`Pane`] only needs to concern itself with the general rendering
/// algorithm.
struct Draw {
    /// Color theme that dictates colors and behaviors.
    theme: ThemeRef,

    /// Current cursor position.
    cursor: Point,

    /// Range in the buffer containing selected text, if applicable, otherwise this
    /// span is assumed to be `0`..`0`.
    select_span: Range<usize>,
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
}

/// Marks the starting point of a selection in the buffer.
///
/// The first value is the buffer position, and the second value is `true` if the
/// mark is *soft*, and `false` if *hard*.
#[derive(Copy, Clone)]
pub struct Mark(pub usize, pub bool);

impl Line {
    /// Returns `true` if the row of this line points to the top of the buffer.
    #[inline]
    fn is_top(&self) -> bool {
        self.row_pos == 0
    }

    /// Returns `true` if the row of this line points to the bottom of the buffer.
    fn is_bottom(&self) -> bool {
        self.line_bottom && !self.does_wrap()
    }

    /// Returns `true` if the row of this line wraps at least to the next row,
    /// indicating that the buffer line is longer than the width of the display.
    #[inline]
    fn does_wrap(&self) -> bool {
        self.row_pos + self.row_len < self.line_pos + self.line_len
    }

    /// Returns `true` if the row of this line has wrapped from at least one prior
    /// row.
    #[inline]
    fn has_wrapped(&self) -> bool {
        self.row_pos > self.line_pos
    }

    /// Returns a possibly smaller value of `col` if it extends beyond the end of
    /// the row.
    ///
    /// In most cases, the right-most column aligns to the last character of the row,
    /// which is usually `\n` but may also be any other character if the row wraps.
    /// However, if this is the bottom-most row in the buffer, there is no terminating
    /// `\n`, and thus the right-most column is right of the last character.
    #[inline]
    fn snap_col(&self, col: u32) -> u32 {
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
        self.row_pos + col as usize
    }

    /// Returns the column number of `pos` relative to the starting position of the
    /// row, though be advised that the resulting column may extend beyond the end
    /// of the row.
    #[inline]
    fn col_of(&self, pos: usize) -> u32 {
        (pos - self.row_pos) as u32
    }

    /// Returns the right-most column number of this row.
    ///
    /// See [snap_col](Line::snap_col) for further details on calculating the
    /// right-most column.
    #[inline]
    fn end_col(&self) -> u32 {
        if self.row_len == 0 {
            0
        } else if self.is_bottom() {
            self.row_len as u32
        } else {
            (self.row_len - 1) as u32
        }
    }

    /// Returns the column number of `col` relative to the buffer line, which may be
    /// larger than the width of the display.
    #[inline]
    fn line_col(&self, col: u32) -> u32 {
        (self.row_pos - self.line_pos) as u32 + col
    }

    /// Returns the buffer position at the end of the row.
    #[inline]
    fn end_pos(&self) -> usize {
        self.row_pos + self.row_len
    }

    fn line_range(&self) -> Range<usize> {
        self.line_pos..(self.line_pos + self.line_len)
    }
}

impl Default for Line {
    fn default() -> Line {
        Line {
            row_pos: 0,
            row_len: 0,
            line_pos: 0,
            line_len: 0,
            line_nbr: 0,
            line_bottom: false,
        }
    }
}

impl Pane {
    fn new(window: WindowRef) -> Pane {
        let theme = window.borrow().theme().clone();
        let canvas = window.borrow().canvas().clone();
        let banner = window.borrow().banner().clone();
        let Size { rows, cols } = canvas.borrow().size();

        Pane {
            theme,
            canvas,
            banner,
            rows,
            cols,
        }
    }

    fn clear(&mut self) {
        self.canvas.borrow_mut().clear();
    }

    fn draw(&mut self, draw: &Draw, p: Point, pos: usize, c: char) -> Option<Point> {
        let mut canvas = self.canvas.borrow_mut();
        let (row, col) = if c == '\n' {
            let color = draw.blank_color(p.row);
            canvas.fill_row_from(p.row, p.col, Cell::new(' ', color));
            (p.row + 1, 0)
        } else {
            let color = draw.cell_color(p, pos);
            canvas.set_cell(p.row, p.col, Cell::new(c, color));
            if p.col + 1 < self.cols {
                (p.row, p.col + 1)
            } else {
                (p.row + 1, 0)
            }
        };
        if row < self.rows {
            Some(Point::new(row, col))
        } else {
            None
        }
    }

    fn draw_rest(&mut self, draw: &Draw, p: Point) -> Option<Point> {
        let mut canvas = self.canvas.borrow_mut();
        let color = draw.blank_color(p.row);
        canvas.fill_row_from(p.row, p.col, Cell::new(' ', color));
        for r in (p.row + 1)..self.rows {
            let color = draw.blank_color(r);
            canvas.fill_row(r, Cell::new(' ', color));
        }
        None
    }

    fn draw_finish(&mut self, draw: &Draw) {
        self.canvas.borrow_mut().draw();
    }

    fn draw_cursor(&mut self, cursor: Point) {
        self.canvas.borrow_mut().set_cursor(cursor);
    }

    fn draw_banner(&mut self, title: String, loc: Point) {
        self.banner
            .borrow_mut()
            .set_title(title)
            .set_cursor(loc)
            .draw();
    }

    fn draw_location(&mut self, loc: Point) {
        self.banner.borrow_mut().set_cursor(loc).draw();
    }
}

impl Draw {
    fn new(editor: &Editor) -> Draw {
        let select_span = editor
            .mark
            .map(|Mark(mark_pos, _)| {
                if mark_pos < editor.cur_pos {
                    mark_pos..editor.cur_pos
                } else {
                    editor.cur_pos..mark_pos
                }
            })
            .unwrap_or(0..0);

        Draw {
            theme: editor.pane.theme.clone(),
            cursor: editor.get_cursor(),
            select_span,
        }
    }

    fn cell_color(&self, p: Point, pos: usize) -> Color {
        if self.select_span.contains(&pos) {
            self.theme.select_color
        } else if self.theme.highlight_row && p.row == self.cursor.row {
            self.theme.highlight_color
        } else {
            self.theme.text_color
        }
    }

    fn blank_color(&self, row: u32) -> Color {
        if self.theme.highlight_row && row == self.cursor.row {
            self.theme.highlight_color
        } else {
            self.theme.text_color
        }
    }
}

impl Editor {
    pub fn new() -> Editor {
        Self::with_path(None)
    }

    pub fn with_path(path: Option<PathBuf>) -> Editor {
        Self::with_buffer(path, Buffer::new().to_ref())
    }

    pub fn with_buffer(path: Option<PathBuf>, buffer: BufferRef) -> Editor {
        let cur_pos = buffer.borrow().get_pos();

        Editor {
            path,
            buffer,
            cur_pos,
            top_line: Line::default(),
            cur_line: Line::default(),
            snap_col: None,
            cursor: Point::ORIGIN,
            mark: None,
            pane: Pane::new(Window::zombie().to_ref()),
        }
    }

    /// Turns the editor into a [`EditorRef`].
    pub fn to_ref(self) -> EditorRef {
        Rc::new(RefCell::new(self))
    }

    #[inline(always)]
    pub fn rows(&self) -> u32 {
        self.pane.rows
    }

    #[inline(always)]
    pub fn cols(&self) -> u32 {
        self.pane.cols
    }

    pub fn get_size(&self) -> Size {
        (self.rows(), self.cols()).into()
    }

    pub fn get_cursor(&self) -> Point {
        self.cursor
    }

    pub fn get_location(&self) -> Point {
        Point::new(
            self.cur_line.line_nbr as u32 + 1,
            self.cur_line.line_col(self.cursor.col) + 1,
        )
    }

    pub fn show_cursor(&mut self) {
        self.pane.draw_cursor(self.cursor);
    }

    /// Attaches the `window` to this editor.
    pub fn attach(&mut self, window: WindowRef) {
        let is_zombie = window.borrow().is_zombie();
        self.pane = Pane::new(window);
        if !is_zombie {
            self.align_cursor(Align::Auto);
        }
    }

    pub fn align_cursor(&mut self, align: Align) {
        // Determine ideal row where cursor would like to be focused, though this should
        // be considered a hint.
        let try_row = match align {
            Align::Auto => cmp::min(self.cursor.row, self.rows() - 1),
            Align::Center => self.rows() / 2,
            Align::Top => 0,
            Align::Bottom => self.rows() - 1,
        };

        // Tries to position cursor on target row, but no guarantee depending on proximity
        // of row to top of buffer.
        self.cur_line = self.find_line(self.cur_pos);
        let row = self.set_top_line(try_row);
        let col = self.cur_line.col_of(self.cur_pos);
        self.snap_col = None;
        self.cursor = Point::new(row, col);
        self.draw();
    }

    fn get_title(&self) -> String {
        match self.path {
            Some(ref path) => path.to_string_lossy().to_string(),
            None => "new".to_string(),
        }
    }

    pub fn draw(&mut self) {
        self.pane.clear();
        self.pane.draw_banner(self.get_title(), self.get_location());
        self.render();
    }

    /// Inserts the character `c` at the current buffer position.
    pub fn insert_char(&mut self, c: char) {
        self.insert(&[c])
    }

    /// Inserts the string slice `str` at the current buffer position.
    pub fn insert_str(&mut self, text: &str) {
        self.insert(&text.chars().collect::<Vec<_>>())
    }

    /// Inserts the array of `text` at the current buffer position.
    pub fn insert(&mut self, text: &[char]) {
        if text.len() > 0 {
            // Most common use case is single-character insertions, so favor use
            // more efficient buffer insertion in that case.
            self.buffer_mut().set_pos(self.cur_pos);
            let cur_pos = if text.len() == 1 {
                self.buffer_mut().insert_char(text[0])
            } else {
                self.buffer_mut().insert(text)
            };

            // Update current line since insertion will have changed critical
            // information for navigation. New cursor location follows inserted text,
            // so need to find new current line. Top line must also be updated even if
            // new cursor location is still visible because insertion may have changed
            // its attributes as well.
            self.cur_line = self.update_line(&self.cur_line);
            let rows = self.find_down_cur_line(cur_pos);
            let row = self.cursor.row + rows;
            let row = if row < self.rows() {
                self.top_line = self.update_line(&self.top_line);
                row
            } else {
                self.set_top_line(self.rows() - 1)
            };
            self.cur_pos = cur_pos;
            let col = self.cur_line.col_of(self.cur_pos);
            self.snap_col = None;
            self.cursor = Point::new(row, col);
            self.render();
        }
    }

    /// Removes and returns the character left of the current buffer position.
    ///
    /// An empty vector is returned if the current position is already at the top
    /// of the buffer.
    pub fn remove_left(&mut self) -> Vec<char> {
        if self.cur_pos > 0 {
            self.remove(self.cur_pos - 1)
        } else {
            vec![]
        }
    }

    /// Removes and returns the character right of the current buffer position.
    ///
    /// An empty vector is returned if the current position is already at the
    /// bottom of the buffer.
    pub fn remove_right(&mut self) -> Vec<char> {
        if self.cur_pos < self.buffer().size() {
            self.remove(self.cur_pos + 1)
        } else {
            vec![]
        }
    }

    /// Removes and returns the text between the current buffer position and `pos`.
    ///
    /// Specifically, the range of characters is bounded *inclusively below* and
    /// *exclusively above*. If `pos` is less than the current buffer position, then
    /// the range is [`pos`, `cur_pos`), otherwise it is [`cur_pos`, `pos`).
    ///
    /// This function will return an empty vector if `pos` is equal to `cur_pos`.
    pub fn remove(&mut self, pos: usize) -> Vec<char> {
        if pos == self.cur_pos {
            vec![]
        } else {
            // Form range depending on location of `pos` relative to current buffer
            // position.
            let pos = cmp::min(pos, self.buffer().size());
            let (from_pos, len) = if pos < self.cur_pos {
                (pos, self.cur_pos - pos)
            } else {
                (self.cur_pos, pos - self.cur_pos)
            };

            // Find new current line which depends on location of `pos` relative to
            // current buffer position. If prior to current position, this requires
            // backtracking since intuition is that resulting cursor would be placed
            // at that lower bound position. Conversely, if following current position,
            // resulting cursor would stay on existing row.
            let row = if from_pos < self.cur_pos {
                let rows = self.find_up_cur_line(from_pos);
                if rows > self.cursor.row {
                    self.set_top_line(0)
                } else {
                    self.cursor.row - rows
                }
            } else {
                self.cursor.row
            };

            // Note that buffer modification comes after finding new current line, and
            // that common use case of single-character removal allows more efficient
            // buffer function to be used.
            self.buffer_mut().set_pos(from_pos);
            let text = if len == 1 {
                vec![self.buffer_mut().remove_char().unwrap()]
            } else {
                self.buffer_mut().remove(len)
            };

            // Removal of text requires current and top lines to be updated since may
            // have changed.
            self.cur_line = self.update_line(&self.cur_line);
            self.top_line = self.update_line(&self.top_line);
            self.cur_pos = from_pos;
            let col = self.cur_line.col_of(self.cur_pos);
            self.snap_col = None;
            self.cursor = Point::new(row, col);
            self.render();
            text
        }
    }

    /// Tries to move the cursor *up* by the specified number of `try_rows`.
    ///
    /// If `pin` is `true`, then the cursor will remain on the current row if the
    /// resulting display makes it possible. Pinning is useful when *paging up*.
    ///
    /// If `pin` is `false`, then the cursor will move up in tandem with `try_rows`,
    /// though not to extend beyond the top of the display.
    pub fn move_up(&mut self, try_rows: u32, pin: bool) {
        let rows = self.up_cur_line(try_rows);
        if rows > 0 {
            let row = if pin {
                if rows < try_rows {
                    // Cursor reached top of buffer before advancing by desired number of
                    // rows, so resulting row is always top of display.
                    self.set_top_line(0)
                } else {
                    // Try finding new top line by stepping backwards by number of rows
                    // equivalent to current row of cursor.
                    self.set_top_line(self.cursor.row)
                }
            } else {
                if rows > self.cursor.row {
                    // Cursor would have moved beyond top of display.
                    self.set_top_line(0)
                } else {
                    // Cursor remains visible without changing top line.
                    self.cursor.row - rows
                }
            };
            let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
            self.snap_col = Some(try_col);
            let col = self.cur_line.snap_col(try_col);
            self.cur_pos = self.cur_line.pos_of(col);
            self.cursor = Point::new(row, col);
            self.render();
        }
    }

    /// Tries to move the cursor *down* by the specified number of `try_rows`.
    ///
    /// If `pin` is `true`, then the cursor will remain on the current row. Pinning is
    /// useful when *paging down*.
    ///
    /// If `pin` is `false`, then the cursor will move down in tandem with `try_rows`,
    /// though not to extend beyond the bottom of the display.
    pub fn move_down(&mut self, try_rows: u32, pin: bool) {
        let rows = self.down_cur_line(try_rows);
        if rows > 0 {
            let row = if pin {
                // Keeping cursor on current row is guaranteed, because top line can
                // always move down without reaching bottom of buffer.
                self.down_top_line(rows);
                self.cursor.row
            } else {
                if self.cursor.row + rows < self.rows() {
                    // Cursor remains visible without changing top line.
                    self.cursor.row + rows
                } else {
                    // Cursor would have moved beyond bottom of display.
                    self.set_top_line(self.rows() - 1)
                }
            };
            let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
            self.snap_col = Some(try_col);
            let col = self.cur_line.snap_col(try_col);
            self.cur_pos = self.cur_line.pos_of(col);
            self.cursor = Point::new(row, col);
            self.render();
        }
    }

    /// Tries to move the cursor *left* of the current buffer position by `len`
    /// characters.
    pub fn move_left(&mut self, len: usize) {
        let pos = self.cur_pos - cmp::min(len, self.cur_pos);
        if pos < self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    /// Tries to move the cursor *right* of the current buffer position by `len`
    /// characters.
    pub fn move_right(&mut self, len: usize) {
        let pos = cmp::min(self.cur_pos + len, self.buffer().size());
        if pos > self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    /// Moves the cursor to the *start* of the current row.
    pub fn move_start(&mut self) {
        if self.cursor.col > 0 {
            self.cur_pos = self.cur_line.row_pos;
            self.cursor.col = 0;
            self.render();
        }
        self.snap_col = None;
    }

    /// Moves the cursor to the *end* of the current row.
    pub fn move_end(&mut self) {
        let end_col = self.cur_line.end_col();
        if self.cursor.col < end_col {
            self.cur_pos = self.cur_line.pos_of(end_col);
            self.cursor.col = end_col;
            self.render();
        }
        self.snap_col = None;
    }

    /// Moves the cursor to the *top* of the buffer.
    pub fn move_top(&mut self) {
        self.move_to(0, Align::Top);
    }

    /// Moves the cursor to the *bottom* of the buffer.
    pub fn move_bottom(&mut self) {
        let pos = self.buffer().size();
        self.move_to(pos, Align::Bottom);
    }

    /// Moves the current buffer position to `pos` and places the cursor on the
    /// display according to the `align` objective.
    ///
    /// When [`Align::Auto`] is specified, the placement of the cursor depends on
    /// the target `pos` relative to the current buffer position. Specifically, it
    /// behaves as follows:
    /// - *when `pos` is above the current line but still visible on the display*:
    ///   aligns the cursor on the target row above the current line, though not to
    ///   extend beyond the top row
    /// - *when `pos` is on the current line*: aligns the cursor on the current row
    /// - *when `pos` is beyond the current line*: aligns the cursor on the target
    ///   row below the current line, though not to extend beyond the borrom row
    pub fn move_to(&mut self, pos: usize, align: Align) {
        let row = if pos < self.top_line.row_pos {
            self.find_up_top_line(pos);
            let rows = match align {
                Align::Top | Align::Auto => 0,
                Align::Center => self.rows() / 2,
                Align::Bottom => self.rows() - 1,
            };
            self.cur_line = self.top_line.clone();
            self.down_cur_line(rows)
        } else if pos < self.cur_line.row_pos {
            let row = self.cursor.row - self.find_up_cur_line(pos);
            let maybe_rows = match align {
                Align::Auto => None,
                Align::Top => Some(0),
                Align::Center => Some(self.rows() / 2),
                Align::Bottom => Some(self.rows() - 1),
            };
            if let Some(rows) = maybe_rows {
                self.set_top_line(rows)
            } else {
                row
            }
        } else if pos < self.cur_line.end_pos() {
            let maybe_rows = match align {
                Align::Auto => None,
                Align::Top => Some(0),
                Align::Center => Some(self.rows() / 2),
                Align::Bottom => Some(self.rows() - 1),
            };
            if let Some(rows) = maybe_rows {
                self.set_top_line(rows)
            } else {
                self.cursor.row
            }
        } else {
            let rows = self.find_down_cur_line(pos);
            let row = match align {
                Align::Auto => cmp::min(self.cursor.row + rows, self.rows() - 1),
                Align::Top => 0,
                Align::Center => self.rows() / 2,
                Align::Bottom => self.rows() - 1,
            };
            self.set_top_line(row)
        };
        self.cur_pos = pos;
        let col = self.cur_line.col_of(self.cur_pos);
        self.snap_col = None;
        self.cursor = Point::new(row, col);
        self.render();
    }

    /// Tries scrolling *up* the contents of the display by the specified number of
    /// `try_rows` while preserving the cursor position, which also means the cursor
    /// moves *up* as the contents scroll.
    pub fn scroll_up(&mut self, try_rows: u32) {
        let rows = self.down_top_line(try_rows);
        if rows > 0 {
            let (row, col) = if rows > self.cursor.row {
                // Cursor would have moved beyond top of display, which means current
                // buffer position changes accordingly.
                self.cur_line = self.top_line.clone();
                let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
                self.snap_col = Some(try_col);
                let col = self.cur_line.snap_col(try_col);
                self.cur_pos = self.cur_line.pos_of(col);
                (0, col)
            } else {
                // Cursor still visible on display.
                (self.cursor.row - rows, self.cursor.col)
            };
            self.cursor = Point::new(row, col);
            self.render();
        }
    }

    /// Tries scrolling *down* the contents of the display by the specified number of
    /// `try_rows` while preserving the cursor position, which also means the cursor
    /// moves *down* as the contents scroll.
    pub fn scroll_down(&mut self, try_rows: u32) {
        let rows = self.up_top_line(try_rows);
        if rows > 0 {
            let row = self.cursor.row + rows;
            let (row, col) = if row < self.rows() {
                // Cursor still visible on display.
                (row, self.cursor.col)
            } else {
                // Cursor would have moved beyond bottom of display, which means current
                // buffer position changes accordingly.
                self.up_cur_line(row - self.rows() + 1);
                let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
                self.snap_col = Some(try_col);
                let col = self.cur_line.snap_col(try_col);
                self.cur_pos = self.cur_line.pos_of(col);
                (self.rows() - 1 as u32, col)
            };
            self.cursor = Point::new(row, col);
            self.render();
        }
    }

    /// Sets a *hard* mark at the current buffer position and returns the previous
    /// mark if set.
    pub fn set_hard_mark(&mut self) -> Option<Mark> {
        self.mark.replace(Mark(self.cur_pos, false))
    }

    /// Sets a *soft* mark at the current buffer position unless a *soft* mark was
    /// previously set.
    ///
    /// Note that if a *hard* mark was previously set, the *soft* mark will replace
    /// it.
    ///
    /// Returns the previous *hard* mark if set, otherwise `None`.
    pub fn set_soft_mark(&mut self) -> Option<Mark> {
        if let Some(mark @ Mark(_, soft)) = self.mark {
            if soft {
                None
            } else {
                self.mark = Some(Mark(self.cur_pos, true));
                Some(mark)
            }
        } else {
            self.mark = Some(Mark(self.cur_pos, true));
            None
        }
    }

    /// Clears and returns the mark if *soft*, otherwise `None` is returned.
    pub fn clear_soft_mark(&mut self) -> Option<Mark> {
        if let Some(Mark(_, true)) = self.mark {
            self.clear_mark()
        } else {
            None
        }
    }

    /// Clears and returns the mark.
    pub fn clear_mark(&mut self) -> Option<Mark> {
        self.mark.take()
    }

    pub fn get_mark_range(&self, mark: Mark) -> Range<usize> {
        let Mark(pos, _) = mark;
        if pos < self.cur_pos {
            pos..self.cur_pos
        } else {
            self.cur_pos..pos
        }
    }

    pub fn copy(&self, mark: Mark) -> Vec<char> {
        let range = self.get_mark_range(mark);
        self.buffer().copy(range.start, range.end)
    }

    pub fn copy_line(&self) -> Vec<char> {
        let range = self.cur_line.line_range();
        self.buffer().copy(range.start, range.end)
    }

    /// Renders the content of the editor.
    pub fn render(&mut self) {
        // Sets up draw context.
        let draw = Draw::new(&self);

        // Renders contents of visible editor contents.
        self.buffer
            .borrow()
            .forward(self.top_line.row_pos)
            .index()
            .try_fold(Point::ORIGIN, |p, (pos, c)| {
                self.pane.draw(&draw, p, pos, c)
            })
            .and_then(|p| self.pane.draw_rest(&draw, p));

        // Finalizes drawing and updates additional meta information.
        self.pane.draw_finish(&draw);
        self.pane.draw_location(self.get_location());
        self.pane.draw_cursor(self.cursor);
    }

    fn set_top_line(&mut self, try_rows: u32) -> u32 {
        self.top_line = self.cur_line.clone();
        self.up_top_line(try_rows)
    }

    fn up_top_line(&mut self, try_rows: u32) -> u32 {
        for rows in 0..try_rows {
            if let Some(line) = self.prev_line(&self.top_line) {
                self.top_line = line;
            } else {
                return rows;
            }
        }
        try_rows
    }

    fn down_top_line(&mut self, try_rows: u32) -> u32 {
        for rows in 0..try_rows {
            if let Some(line) = self.next_line(&self.top_line) {
                self.top_line = line;
            } else {
                return rows;
            }
        }
        try_rows
    }

    fn find_up_top_line(&mut self, pos: usize) -> u32 {
        let mut rows = 0;
        while pos < self.top_line.row_pos {
            self.top_line = self.prev_line_unchecked(&self.top_line);
            rows += 1;
        }
        rows
    }

    fn find_up_cur_line(&mut self, pos: usize) -> u32 {
        let mut rows = 0;
        while pos < self.cur_line.row_pos {
            self.cur_line = self.prev_line_unchecked(&self.cur_line);
            rows += 1;
        }
        rows
    }

    fn up_cur_line(&mut self, try_rows: u32) -> u32 {
        for rows in 0..try_rows {
            if let Some(line) = self.prev_line(&self.cur_line) {
                self.cur_line = line;
            } else {
                return rows;
            }
        }
        try_rows
    }

    fn down_cur_line(&mut self, try_rows: u32) -> u32 {
        for rows in 0..try_rows {
            if let Some(line) = self.next_line(&self.cur_line) {
                self.cur_line = line;
            } else {
                return rows;
            }
        }
        try_rows
    }

    fn find_down_cur_line(&mut self, pos: usize) -> u32 {
        let mut rows = 0;
        while pos >= self.cur_line.end_pos() && !self.cur_line.is_bottom() {
            self.cur_line = self.next_line_unchecked(&self.cur_line);
            rows += 1;
        }
        rows
    }

    /// Finds and returns the display line corresponding to `pos`.
    fn find_line(&self, pos: usize) -> Line {
        let (line_pos, next_pos, terminated) = self.find_line_bounds(pos);
        let line_len = next_pos - line_pos;
        let row_pos = pos - ((pos - line_pos) % self.cols() as usize);
        let row_len = cmp::min(line_len - (row_pos - line_pos), self.cols() as usize);
        Line {
            row_pos,
            row_len,
            line_pos,
            line_len,
            line_nbr: self.buffer().line_of(line_pos),
            line_bottom: !terminated,
        }
    }

    /// Returns an updated `line` based on the assumption of underlying changes to
    /// the buffer.
    ///
    /// Note that none of `line_pos`, `row_pos`, and `line_nbr` are modified as part
    /// of this update, as those are presumed to have not changed.
    ///
    /// The rationale for this function is that an insertion or deletion of text is
    /// always relative to the current line, and that such a change would never
    /// alter the values noted above.
    fn update_line(&self, line: &Line) -> Line {
        let (next_pos, terminated) = self.buffer().find_next_line(line.line_pos);
        let line_len = next_pos - line.line_pos;
        let row_len = cmp::min(
            line_len - (line.row_pos - line.line_pos),
            self.cols() as usize,
        );
        Line {
            row_len,
            line_len,
            line_bottom: !terminated,
            ..*line
        }
    }

    /// Returns the line preceding `line`, or `None` if `line` is already at the
    /// top of the buffer.
    fn prev_line(&self, line: &Line) -> Option<Line> {
        if line.is_top() {
            None
        } else if line.has_wrapped() {
            let l = Line {
                row_pos: line.row_pos - self.cols() as usize,
                row_len: self.cols() as usize,
                ..*line
            };
            Some(l)
        } else {
            let pos = line.line_pos - 1;
            let (line_pos, next_pos, terminated) = self.find_line_bounds(pos);
            let line_len = next_pos - line_pos;
            let row_pos = pos - ((pos - line_pos) % self.cols() as usize);
            let row_len = cmp::min(line_len - (row_pos - line_pos), self.cols() as usize);
            let l = Line {
                row_pos,
                row_len,
                line_pos,
                line_len,
                line_nbr: line.line_nbr - 1,
                line_bottom: !terminated,
            };
            Some(l)
        }
    }

    /// An unchecked version of [prev_line](Editor::prev_line) that assumes `line`
    /// is not at the top of the buffer.
    fn prev_line_unchecked(&self, line: &Line) -> Line {
        self.prev_line(line)
            .unwrap_or_else(|| panic!("line already at top of buffer"))
    }

    /// Returns the line following `line`, or `None` if `line` is already at the
    /// bottom of the buffer.
    fn next_line(&self, line: &Line) -> Option<Line> {
        if line.is_bottom() {
            None
        } else if line.does_wrap() {
            let row_pos = line.row_pos + line.row_len;
            let row_len = cmp::min(
                line.line_len - (row_pos - line.line_pos),
                self.cols() as usize,
            );
            let l = Line {
                row_pos,
                row_len,
                ..*line
            };
            Some(l)
        } else {
            let line_pos = line.line_pos + line.line_len;
            let (next_pos, terminated) = self.buffer().find_next_line(line_pos);
            let line_len = next_pos - line_pos;
            let row_len = cmp::min(line_len, self.cols() as usize);
            let l = Line {
                row_pos: line_pos,
                row_len,
                line_pos,
                line_len,
                line_nbr: line.line_nbr + 1,
                line_bottom: !terminated,
            };
            Some(l)
        }
    }

    /// An unchecked version of [next_line](Editor::next_line) that assumes `line`
    /// is not at the bottom of the buffer.
    fn next_line_unchecked(&self, line: &Line) -> Line {
        self.next_line(line)
            .unwrap_or_else(|| panic!("line already at bottom of buffer"))
    }

    /// Returns a tuple, relative to the buffer line corresponding to `pos`, containing
    /// the position of the first character on that line, the position of the first
    /// character of the next line, and a boolean value indicating if that the line was
    /// terminated with `\n`.
    fn find_line_bounds(&self, pos: usize) -> (usize, usize, bool) {
        let buffer = self.buffer.borrow();
        let line_pos = buffer.find_start_line(pos);
        let (next_pos, terminated) = buffer.find_next_line(pos);
        (line_pos, next_pos, terminated)
    }

    #[inline]
    fn buffer(&self) -> Ref<'_, Buffer> {
        self.buffer.borrow()
    }

    #[inline]
    fn buffer_mut(&self) -> RefMut<'_, Buffer> {
        self.buffer.borrow_mut()
    }
}
