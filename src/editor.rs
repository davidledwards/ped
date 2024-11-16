//! Editor.
use crate::buffer::{Buffer, BufferRef};
use crate::canvas::{Canvas, CanvasRef};
use crate::grid::Cell;
use crate::size::{Point, Size};
use crate::theme::{Theme, ThemeRef};
use crate::window::{Banner, BannerRef, Window, WindowRef};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;
use std::time::SystemTime;

/// An editing controller with an underlying [`Buffer`] and an attachable
/// [`Window`].
pub struct Editor {
    /// Type of storage associated with this editor.
    storage: Storage,

    /// Buffer containing the contents of this editor.
    buffer: BufferRef,

    /// An indication that unsaved changes have been made to the buffer.
    dirty: bool,

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

    /// Color theme that applies to the window.
    theme: ThemeRef,

    /// Canvas associated with the window.
    canvas: CanvasRef,

    /// Banner associated with the window.
    banner: BannerRef,

    /// Number of rows available for text.
    rows: u32,

    /// Number of columns available for text.
    cols: u32,

    /// Number of columns allocated to the margin for displaying line numbers.
    margin_cols: u32,
}

pub type EditorRef = Rc<RefCell<Editor>>;

/// The storage types associated with an [`Editor`].
#[derive(Clone)]
pub enum Storage {
    /// A type of storage indicating that the buffer is stored and may be written to
    /// a persistent medium.
    Persistent {
        path: String,
        time: Option<SystemTime>,
    },

    /// A type of storage indicating that the buffer can be discarded.
    Transient { name: String },
}

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
    line: u32,

    /// Indicates that the buffer line is the bottom-most line in the buffer.
    line_bottom: bool,
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

/// Marks the starting point of a selection in the buffer.
///
/// The first value is the buffer position, and the second value is `true` if the
/// mark is *soft*, and `false` if *hard*.
#[derive(Copy, Clone)]
pub struct Mark(pub usize, pub bool);

/// A drawing context provided to rendering functions.
struct Draw {
    /// Color theme that dictates colors and behaviors.
    theme: ThemeRef,

    /// Current cursor position.
    cursor: Point,

    /// Range in the buffer containing selected text, if applicable, otherwise this
    /// span is assumed to be `0`..`0`.
    select_span: Range<usize>,
}

/// A rendering context that captures state information for rendering functions.
struct Render {
    pos: usize,
    row: u32,
    col: u32,
    line: u32,
    line_wrapped: bool,
}

impl Storage {
    pub fn as_persistent(path: &str, time: Option<SystemTime>) -> Storage {
        Storage::Persistent {
            path: path.to_string(),
            time,
        }
    }

    pub fn as_transient(name: &str) -> Storage {
        Storage::Transient {
            name: name.to_string(),
        }
    }

    pub fn path(&self) -> Option<String> {
        match self {
            Storage::Persistent { ref path, time: _ } => Some(path.clone()),
            _ => None,
        }
    }
}

impl fmt::Display for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Storage::Persistent { path, time } => {
                if let Some(_) = time {
                    write!(f, "{path}")
                } else {
                    write!(f, "{path} (new)")
                }
            }
            Storage::Transient { name } => write!(f, "@{name}"),
        }
    }
}

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
    /// See [`snap_col`](Line::snap_col) for further details on calculating the
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
            line: 0,
            line_bottom: false,
        }
    }
}

impl Draw {
    const EOL_CHAR: char = '\u{23ce}';

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
            theme: editor.theme.clone(),
            cursor: editor.cursor(),
            select_span,
        }
    }

    #[inline]
    fn as_line(&self, c: char) -> Cell {
        Cell::new(c, self.theme.line_color)
    }

    #[inline]
    fn as_blank(&self, c: char) -> Cell {
        Cell::new(c, self.theme.text_color)
    }

    #[inline]
    fn convert_char(&self, c: char) -> char {
        if c == '\n' {
            if self.theme.show_eol {
                Self::EOL_CHAR
            } else {
                ' '
            }
        } else {
            c
        }
    }

    fn as_text(&self, c: char, render: &Render) -> Cell {
        let color = if self.select_span.contains(&render.pos) {
            self.theme.select_color
        } else if self.theme.highlight_row && render.row == self.cursor.row {
            self.theme.highlight_color
        } else {
            if c == '\n' && self.theme.show_eol {
                self.theme.eol_color
            } else {
                self.theme.text_color
            }
        };
        Cell::new(self.convert_char(c), color)
    }
}

impl Render {
    /// Creates an initial rendering context from `editor`.
    fn new(editor: &Editor) -> Render {
        Render {
            pos: editor.top_line.row_pos,
            row: 0,
            col: 0,
            line: editor.top_line.line + 1,
            line_wrapped: false,
        }
    }

    /// Returns a new rendering context representing a transition to the next column.
    fn next_col(&self) -> Render {
        Render {
            pos: self.pos + 1,
            col: self.col + 1,
            ..*self
        }
    }

    /// Returns a new rendering context representing a transition to the next row,
    /// indicating that the current line wraps.
    fn next_row(&self) -> Render {
        Render {
            pos: self.pos + 1,
            row: self.row + 1,
            col: 0,
            line_wrapped: true,
            ..*self
        }
    }

    /// Returns a new rendering context representing a transition to the next line,
    /// which is also the next row.
    fn next_line(&self) -> Render {
        Render {
            pos: self.pos + 1,
            row: self.row + 1,
            col: 0,
            line: self.line + 1,
            line_wrapped: false,
        }
    }
}

impl Editor {
    /// Number of columns allocated to the margin for displaying line numbers.
    const MARGIN_COLS: u32 = 6;

    /// Exclusive upper bound on line numbers that can be displayed in the margin.
    const LINE_LIMIT: u32 = 10_u32.pow(Self::MARGIN_COLS - 1);

    pub fn new(storage: Storage, buffer: BufferRef) -> Editor {
        let cur_pos = buffer.borrow().get_pos();
        Editor {
            storage,
            buffer,
            dirty: false,
            cur_pos,
            top_line: Line::default(),
            cur_line: Line::default(),
            snap_col: None,
            cursor: Point::ORIGIN,
            mark: None,
            theme: Theme::default().to_ref(),
            canvas: Canvas::zero().to_ref(),
            banner: Banner::none().to_ref(),
            rows: 0,
            cols: 0,
            margin_cols: 0,
        }
    }

    /// Turns the editor into a [`EditorRef`].
    pub fn to_ref(self) -> EditorRef {
        Rc::new(RefCell::new(self))
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    #[inline]
    pub fn buffer(&self) -> Ref<'_, Buffer> {
        self.buffer.borrow()
    }

    #[inline]
    fn buffer_mut(&self) -> RefMut<'_, Buffer> {
        self.buffer.borrow_mut()
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self, storage: Storage) {
        self.storage = storage;
        self.dirty = false;
        self.show_banner();
    }

    /// Returns the cursor position on the display in terms of *row* and *column*.
    ///
    /// The *row* and *column* values are `0`-based and exclusively bounded by
    /// [`rows`](Self::rows) and [`cols`](Self::cols), respectively.
    pub fn cursor(&self) -> Point {
        self.cursor
    }

    /// Returns the cursor position in the buffer, which is not necessarily the same
    /// as [`Buffer::get_pos`] since changes to the buffer position are deferred until
    /// mutations are applied.
    pub fn cursor_pos(&self) -> usize {
        self.cur_pos
    }

    /// Returns the location of the cursor position in the buffer in terms of *line*
    /// and *column*.
    ///
    /// The *line* and *column* values are `0`-based. Note that neither of these values
    /// are bounded by the size of the display, which is the case with
    /// [`cursor`](Self::cursor).
    pub fn location(&self) -> Point {
        Point::new(self.cur_line.line, self.cur_line.line_col(self.cursor.col))
    }

    #[inline(always)]
    pub fn rows(&self) -> u32 {
        self.rows
    }

    #[inline(always)]
    pub fn cols(&self) -> u32 {
        self.cols
    }

    pub fn size(&self) -> Size {
        (self.rows, self.cols).into()
    }

    /// Attaches the `window` to this editor.
    pub fn attach(&mut self, window: WindowRef, align: Align) {
        let is_zombie = window.borrow().is_zombie();
        self.theme = window.borrow().theme().clone();
        self.canvas = window.borrow().canvas().clone();
        self.banner = window.borrow().banner().clone();

        // Allocate leftmost columns of window to line numbers, but only if enabled and
        // total width of window is large enough to reasonably accommodate.
        let Size { rows, cols } = self.canvas.borrow().size();
        self.margin_cols = if self.theme.show_lines && cols >= Self::MARGIN_COLS * 2 {
            Self::MARGIN_COLS
        } else {
            0
        };
        self.rows = rows;
        self.cols = cols - self.margin_cols;

        if !is_zombie {
            self.align_cursor(align);
        }
    }

    /// Detaches the existing window from this editor.
    pub fn detach(&mut self) {
        self.attach(Window::zombie().to_ref(), Align::Auto);
    }

    pub fn align_cursor(&mut self, align: Align) {
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
        self.cur_line = self.find_line(self.cur_pos);
        let row = self.set_top_line(try_row);
        let col = self.cur_line.col_of(self.cur_pos);
        self.snap_col = None;
        self.cursor = Point::new(row, col);
        self.draw();
    }

    pub fn draw(&mut self) {
        self.canvas.borrow_mut().clear();
        self.show_banner();
        self.render();
    }

    pub fn show_cursor(&mut self) {
        let cursor = if self.margin_cols > 0 {
            self.cursor + Size::cols(self.margin_cols)
        } else {
            self.cursor
        };
        self.canvas.borrow_mut().set_cursor(cursor);
    }

    fn show_banner(&mut self) {
        self.banner
            .borrow_mut()
            .set_dirty(self.dirty)
            .set_title(self.storage.to_string())
            .set_location(self.location())
            .draw();
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
                if self.cursor.row + rows < self.rows {
                    // Cursor remains visible without changing top line.
                    self.cursor.row + rows
                } else {
                    // Cursor would have moved beyond bottom of display.
                    self.set_top_line(self.rows - 1)
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

    /// Moves the buffer position to the first character of `line` and places the
    /// cursor on the display according to the `align` objective.
    pub fn move_line(&mut self, line: u32, align: Align) {
        let pos = self.buffer().find_line(line);
        self.move_to(pos, align);
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
            self.find_up_cur_line(pos);
            let rows = match align {
                Align::Top | Align::Auto => 0,
                Align::Center => self.rows / 2,
                Align::Bottom => self.rows - 1,
                Align::Row(row) => cmp::min(row, self.rows - 1),
            };
            self.set_top_line(rows)
        } else if pos < self.cur_line.row_pos {
            let row = self.cursor.row - self.find_up_cur_line(pos);
            let maybe_rows = match align {
                Align::Auto => None,
                Align::Top => Some(0),
                Align::Center => Some(self.rows / 2),
                Align::Bottom => Some(self.rows - 1),
                Align::Row(row) => Some(cmp::min(row, self.rows - 1)),
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
                Align::Center => Some(self.rows / 2),
                Align::Bottom => Some(self.rows - 1),
                Align::Row(row) => Some(cmp::min(row, self.rows - 1)),
            };
            if let Some(rows) = maybe_rows {
                self.set_top_line(rows)
            } else {
                self.cursor.row
            }
        } else {
            let rows = self.find_down_cur_line(pos);
            let row = match align {
                Align::Auto => cmp::min(self.cursor.row + rows, self.rows - 1),
                Align::Top => 0,
                Align::Center => self.rows / 2,
                Align::Bottom => self.rows - 1,
                Align::Row(row) => cmp::min(row, self.rows - 1),
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
            let (row, col) = if row < self.rows {
                // Cursor still visible on display.
                (row, self.cursor.col)
            } else {
                // Cursor would have moved beyond bottom of display, which means current
                // buffer position changes accordingly.
                self.up_cur_line(row - self.rows + 1);
                let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
                self.snap_col = Some(try_col);
                let col = self.cur_line.snap_col(try_col);
                self.cur_pos = self.cur_line.pos_of(col);
                (self.rows - 1 as u32, col)
            };
            self.cursor = Point::new(row, col);
            self.render();
        }
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
            let row = if row < self.rows {
                self.top_line = self.update_line(&self.top_line);
                row
            } else {
                self.set_top_line(self.rows - 1)
            };
            self.cur_pos = cur_pos;
            let col = self.cur_line.col_of(self.cur_pos);
            self.snap_col = None;
            self.cursor = Point::new(row, col);
            self.dirty = true;
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

    /// Removes and returns the text between the current buffer position and `mark`.
    pub fn remove_mark(&mut self, mark: Mark) -> Vec<char> {
        let Mark(pos, _) = mark;
        self.remove(pos)
    }

    /// Removes and returns the text of the line on which the current buffer position
    /// rests.
    pub fn remove_line(&mut self) -> Vec<char> {
        let Range { start, end } = self.cur_line.line_range();
        self.move_to(start, Align::Auto);
        self.remove(end)
    }

    /// Removes and returns the text between the start of the current line and the
    /// current buffer position.
    pub fn remove_start(&mut self) -> Vec<char> {
        if self.cur_pos == self.cur_line.line_pos {
            self.remove_left()
        } else {
            self.remove(self.cur_line.line_pos)
        }
    }

    /// Removes and returns the text between the current buffer position and the end
    /// of the current line.
    pub fn remove_end(&mut self) -> Vec<char> {
        self.remove(self.cur_line.line_pos + self.cur_line.line_len)
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
            self.dirty = true;
            self.render();
            text
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

    fn get_mark_range(&self, mark: Mark) -> Range<usize> {
        let Mark(pos, _) = mark;
        if pos < self.cur_pos {
            pos..self.cur_pos
        } else {
            self.cur_pos..pos
        }
    }

    /// Returns the text between the current buffer position and `mark`.
    pub fn copy_mark(&self, mark: Mark) -> Vec<char> {
        let Range { start, end } = self.get_mark_range(mark);
        self.copy(start, end)
    }

    /// Returns the text of the line on which the current buffer position rests.
    pub fn copy_line(&self) -> Vec<char> {
        let Range { start, end } = self.cur_line.line_range();
        self.copy(start, end)
    }

    /// Returns the text between `from_pos` and `end_pos`.
    ///
    /// Specifically, the range of characters is bounded *inclusively below* and
    /// *exclusively above*. If `from_pos` is less than `to_pos`, then the range is
    /// [`from_pos`, `to_pos`), otherwise it is [`to_pos`, `from_pos`).
    ///
    /// This function will return an empty vector if `from_pos` is equal to `to_pos`.
    pub fn copy(&self, from_pos: usize, to_pos: usize) -> Vec<char> {
        self.buffer().copy(from_pos, to_pos)
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
        let row_pos = pos - ((pos - line_pos) % self.cols as usize);
        let row_len = cmp::min(line_len - (row_pos - line_pos), self.cols as usize);
        Line {
            row_pos,
            row_len,
            line_pos,
            line_len,
            line: self.buffer().line_of(line_pos),
            line_bottom: !terminated,
        }
    }

    /// Returns an updated `line` based on the assumption of underlying changes to
    /// the buffer.
    ///
    /// Note that none of `line_pos`, `row_pos`, and `line` are modified as part of
    /// this update, as those are presumed to have not changed.
    ///
    /// The rationale for this function is that an insertion or deletion of text is
    /// always relative to the current line, and that such a change would never
    /// alter the values noted above.
    fn update_line(&self, line: &Line) -> Line {
        let (next_pos, terminated) = self.buffer().find_next_line(line.line_pos);
        let line_len = next_pos - line.line_pos;
        let row_len = cmp::min(
            line_len - (line.row_pos - line.line_pos),
            self.cols as usize,
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
                row_pos: line.row_pos - self.cols as usize,
                row_len: self.cols as usize,
                ..*line
            };
            Some(l)
        } else {
            let pos = line.line_pos - 1;
            let (line_pos, next_pos, terminated) = self.find_line_bounds(pos);
            let line_len = next_pos - line_pos;
            let row_pos = pos - ((pos - line_pos) % self.cols as usize);
            let row_len = cmp::min(line_len - (row_pos - line_pos), self.cols as usize);
            let l = Line {
                row_pos,
                row_len,
                line_pos,
                line_len,
                line: line.line - 1,
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
                self.cols as usize,
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
            let row_len = cmp::min(line_len, self.cols as usize);
            let l = Line {
                row_pos: line_pos,
                row_len,
                line_pos,
                line_len,
                line: line.line + 1,
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

    /// Renders the contents of the editor.
    pub fn render(&mut self) {
        // Renders visible buffer content.
        let draw = Draw::new(&self);
        let render = Render::new(&self);
        let rest = self
            .buffer
            .borrow()
            .forward(render.pos)
            .try_fold(render, |render, c| self.render_cell(&draw, &render, c));
        if let Some(render) = rest {
            self.render_rest(&draw, &render);
        }
        self.canvas.borrow_mut().draw();

        // Renders additional information.
        self.banner
            .borrow_mut()
            .set_dirty(self.dirty)
            .set_location(self.location())
            .draw();
    }

    /// Renders an individual cell for the character `c`, returning the next rendering
    /// context or `None` if rendering has finished.
    fn render_cell(&self, draw: &Draw, render: &Render, c: char) -> Option<Render> {
        self.render_margin(draw, render);
        let mut canvas = self.canvas.borrow_mut();
        let (row, col) = (render.row, render.col + self.margin_cols);
        let render = if c == '\n' {
            canvas.set_cell(row, col, draw.as_text(c, render));
            canvas.fill_row_from(row, col + 1, draw.as_text(' ', render));
            render.next_line()
        } else {
            canvas.set_cell(row, col, draw.as_text(c, render));
            if render.col + 1 < self.cols {
                render.next_col()
            } else {
                render.next_row()
            }
        };
        if render.row < self.rows {
            Some(render)
        } else {
            None
        }
    }

    /// Renders the remainder of the displayable area which is considered empty space.
    ///
    /// This function gets invoked when the end of buffer is reached before the entire
    /// canvas is rendered.
    fn render_rest(&self, draw: &Draw, render: &Render) {
        self.render_margin(draw, render);
        let mut canvas = self.canvas.borrow_mut();

        // Blank out rest of existing row.
        let (row, col) = (render.row, render.col + self.margin_cols);
        canvas.fill_row_from(row, col, draw.as_text(' ', render));

        // Blank out remaining rows.
        for row in (render.row + 1)..self.rows {
            if self.margin_cols > 0 {
                canvas.fill_row_range(row, 0, self.margin_cols, draw.as_line(' '));
            }
            canvas.fill_row_from(row, self.margin_cols, draw.as_blank(' '));
        }
    }

    /// Renders the margin if line numbering is enabled and the rendering context is
    /// on the first column of any row.
    fn render_margin(&self, draw: &Draw, render: &Render) {
        if render.col == 0 && self.margin_cols > 0 {
            let mut canvas = self.canvas.borrow_mut();
            if render.line_wrapped {
                canvas.fill_row_range(render.row, 0, self.margin_cols, draw.as_line(' '));
            } else if render.line < Self::LINE_LIMIT {
                let s = format!(
                    "{:>cols$} ",
                    render.line,
                    cols = Self::MARGIN_COLS as usize - 1
                );
                for (col, c) in s.char_indices() {
                    canvas.set_cell(render.row, col as u32, draw.as_line(c));
                }
            } else {
                canvas.fill_row_range(render.row, 0, self.margin_cols - 1, draw.as_line('-'));
                canvas.set_cell(render.row, self.margin_cols, draw.as_line(' '));
            }
        }
    }
}

pub fn persistent(path: &str, time: Option<SystemTime>, buffer: Option<Buffer>) -> EditorRef {
    let buffer = buffer.unwrap_or_else(|| Buffer::new()).to_ref();
    Editor::new(Storage::as_persistent(path, time), buffer).to_ref()
}

pub fn transient(name: &str, buffer: Option<Buffer>) -> EditorRef {
    let buffer = buffer.unwrap_or_else(|| Buffer::new()).to_ref();
    Editor::new(Storage::as_transient(name), buffer).to_ref()
}
