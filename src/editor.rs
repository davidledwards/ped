//! Provides a core set of editing functions over a buffer and an attachable window.
//!
//! An editor coordinates changes to and movement within a buffer and renders those
//! effects on the display of an attached window. A majority of the work in this module
//! is focused on display rendering.

use crate::buffer::{Buffer, BufferRef};
use crate::canvas::{Canvas, CanvasRef};
use crate::color::Color;
use crate::config::ConfigurationRef;
use crate::grid::Cell;
use crate::size::{Point, Size};
use crate::source::Source;
use crate::syntax::Syntax;
use crate::token::{Cursor, Tokenizer, TokenizerRef};
use crate::window::{Banner, BannerRef, Window, WindowRef};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp;
use std::ops::Range;
use std::rc::Rc;

/// An editing controller with an underlying [`Buffer`] and an attachable [`Window`].
pub struct Editor {
    /// Global configuration.
    config: ConfigurationRef,

    /// The source of the buffer.
    source: Source,

    /// Buffer containing the contents of this editor.
    buffer: BufferRef,

    /// A stack containing changes to the buffer that can be *undone*.
    undo: Vec<Change>,

    /// A stack containing changes to the buffer that can be *redone*.
    redo: Vec<Change>,

    /// Tokenizes the buffer for syntax coloring.
    tokenizer: TokenizerRef,

    /// A tokenization cursor that is always pointing to the top-left position on the
    /// display.
    syntax_cursor: Cursor,

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

/// The distinct types of changes to a buffer recorded in the *undo* and *redo* stacks.
enum Change {
    /// Represents the insertion of text, where values are defined as:
    /// - buffer position prior to insertion
    /// - text inserted
    Insert(usize, Vec<char>),

    /// Represents the removal of text that comes before the cursor, where values are
    /// defined as:
    /// - buffer position prior to removal
    /// - text removed
    RemoveBefore(usize, Vec<char>),

    /// Represents the removal of text that comes after the cursor, where values are
    /// defined as:
    /// - buffer position prior to removal
    /// - text removed
    RemoveAfter(usize, Vec<char>),

    /// Represents the removal of selected text that comes before the cursor, where
    /// values are defined as:
    /// - buffer position prior to removal
    /// - text removed
    RemoveSelectionBefore(usize, Vec<char>, bool),

    /// Represents the removal of selected text that comes after the cursor, where
    /// values are defined as:
    /// - buffer position prior to removal
    /// - text removed
    RemoveSelectionAfter(usize, Vec<char>, bool),
}

/// Indicates how a [`Change`] should be logged.
enum Log {
    /// Indicates that no selection was active when the change was made.
    Normal,

    /// Indicates that a selection was active when the change was made, where the
    /// value is `true` if it was a *soft* mark and `false` if a *hard* mark.
    Selection(bool),
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
    /// Configuration that dictates colors and behaviors.
    config: ConfigurationRef,

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
    tokenizer: TokenizerRef,
    syntax_cursor: Cursor,
}

impl Change {
    /// Returns a new change if `self` can be combined with `prior`, otherwise `None`.
    ///
    /// In general, this function is used to optimize changes that involve a single
    /// character being inserted or removed. If the change described by `self` is
    /// adjacent to `prior`, then both changes are combined into a single change.
    fn possibly_combine(&self, prior: &Change) -> Option<Change> {
        use Change::{Insert, RemoveAfter, RemoveBefore};

        match self {
            Insert(pos, text) if text.len() == 1 => match prior {
                Insert(p_pos, p_text) if p_pos + p_text.len() == *pos => {
                    let mut p_text = p_text.clone();
                    p_text.push(text[0]);
                    Some(Insert(*p_pos, p_text))
                }
                _ => None,
            },
            RemoveBefore(pos, text) if text.len() == 1 => match prior {
                RemoveBefore(p_pos, p_text) if pos + p_text.len() == *p_pos => {
                    let mut p_text = p_text.clone();
                    p_text.insert(0, text[0]);
                    Some(RemoveBefore(*p_pos, p_text))
                }
                _ => None,
            },
            RemoveAfter(pos, text) if text.len() == 1 => match prior {
                RemoveAfter(p_pos, p_text) if *p_pos == *pos => {
                    let mut p_text = p_text.clone();
                    p_text.push(text[0]);
                    Some(RemoveAfter(*p_pos, p_text))
                }
                _ => None,
            },
            _ => None,
        }
    }
}

impl Line {
    /// Returns `true` if the row of this line points to the top of the buffer.
    #[inline]
    fn is_top(&self) -> bool {
        self.row_pos == 0
    }

    /// Returns `true` if the row of this line points to the bottom of the buffer,
    /// where `cols` is the width of the display.
    fn is_bottom(&self, cols: u32) -> bool {
        self.line_bottom && self.row_len < cols as usize
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
    /// the row, where `cols` is the width of the display.
    ///
    /// In most cases, the right-most column aligns to the last character of the row,
    /// which is usually `\n` but may also be any other character if the row wraps.
    /// However, if this is the bottom-most row in the buffer, there is no terminating
    /// `\n`, and thus the right-most column is right of the last character.
    #[inline]
    fn snap_col(&self, col: u32, cols: u32) -> u32 {
        if self.row_len == 0 {
            0
        } else if self.is_bottom(cols) {
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

    /// Returns the right-most column number of this row, where `cols` is the width of
    /// the display.
    ///
    /// See [`snap_col`](Line::snap_col) for further details on calculating the
    /// right-most column.
    #[inline]
    fn end_col(&self, cols: u32) -> u32 {
        if self.row_len == 0 {
            0
        } else if self.is_bottom(cols) {
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
    const EOL_CHAR: char = '\u{21b2}';
    const TAB_CHAR: char = '\u{2192}';

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
            config: editor.config.clone(),
            cursor: editor.cursor(),
            select_span,
        }
    }

    /// Formats `c` using the margin color.
    #[inline]
    fn as_margin(&self, c: char) -> Cell {
        Cell::new(c, self.config.theme.margin_color)
    }

    /// Formats ` ` (space) using the text color.
    #[inline]
    fn as_blank(&self) -> Cell {
        Cell::new(' ', self.config.theme.text_color)
    }

    /// Formats `c` using a color depending on the current rendering context.
    fn as_text(&self, c: char, render: &Render) -> Cell {
        let fg = if let Some(fg) = render.syntax_cursor.color() {
            fg
        } else {
            if (c == '\n' && self.config.settings.eol) || c == '\t' {
                self.config.theme.whitespace_fg
            } else {
                self.config.theme.text_fg
            }
        };

        let bg = if self.select_span.contains(&render.pos) {
            self.config.theme.select_bg
        } else if self.config.settings.spotlight && render.row == self.cursor.row {
            self.config.theme.spotlight_bg
        } else {
            self.config.theme.text_bg
        };

        Cell::new(self.convert_char(c), Color::new(fg, bg))
    }

    /// Converts `c`, if `\n`, to its displayable form, otherwise `c` is returned
    /// unchanged.
    #[inline]
    fn convert_char(&self, c: char) -> char {
        if c == '\n' {
            if self.config.settings.eol {
                Self::EOL_CHAR
            } else {
                ' '
            }
        } else if c == '\t' {
            Self::TAB_CHAR
        } else {
            c
        }
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
            tokenizer: editor.tokenizer.clone(),
            syntax_cursor: editor.syntax_cursor,
        }
    }

    /// Returns a new rendering context representing a transition to the next column.
    fn next_col(self) -> Render {
        Render {
            pos: self.pos + 1,
            col: self.col + 1,
            syntax_cursor: self.syntax_forward(1),
            ..self
        }
    }

    /// Returns a new rendering context representing a transition to the next row,
    /// indicating that the current line wraps.
    fn next_row(self) -> Render {
        Render {
            pos: self.pos + 1,
            row: self.row + 1,
            col: 0,
            line_wrapped: true,
            syntax_cursor: self.syntax_forward(1),
            ..self
        }
    }

    /// Returns a new rendering context representing a transition to the next line,
    /// which is also the next row.
    fn next_line(self) -> Render {
        Render {
            pos: self.pos + 1,
            row: self.row + 1,
            col: 0,
            line: self.line + 1,
            line_wrapped: false,
            syntax_cursor: self.syntax_forward(1),
            ..self
        }
    }

    /// Returns a new syntax cursor moved forward by `n` characters.
    fn syntax_forward(&self, n: usize) -> Cursor {
        self.tokenizer.borrow().forward(self.syntax_cursor, n)
    }
}

impl Editor {
    /// Number of columns allocated to the margin.
    const MARGIN_COLS: u32 = 6;

    /// Exclusive upper bound on line numbers that can be displayed in the margin.
    const LINE_LIMIT: u32 = 10_u32.pow(Self::MARGIN_COLS - 1);

    /// Creates a new editor using `source` and an optional `buffer`.
    ///
    /// If `buffer` is `None`, then an empty buffer is created.
    pub fn new(config: ConfigurationRef, source: Source, buffer: Option<Buffer>) -> Editor {
        let buffer = buffer.unwrap_or_else(|| Buffer::new()).to_ref();
        let cur_pos = buffer.borrow().get_pos();

        // Constructs syntax configuration based on type of buffer and file extension,
        // if applicable.
        let syntax = if let Source::File(path, _) = &source {
            config
                .registry
                .find(path)
                .map(|syntax| syntax.clone())
                .unwrap_or_else(|| Syntax::default())
        } else {
            Syntax::default()
        };

        // Tokenize buffer.
        let mut tokenizer = Tokenizer::new(syntax);
        let syntax_cursor = tokenizer.tokenize(&buffer.borrow());

        Editor {
            config,
            source,
            buffer,
            undo: Vec::new(),
            redo: Vec::new(),
            tokenizer: tokenizer.to_ref(),
            syntax_cursor,
            dirty: false,
            cur_pos,
            top_line: Line::default(),
            cur_line: Line::default(),
            snap_col: None,
            cursor: Point::ORIGIN,
            mark: None,
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

    /// Returns a partial clone of this editor using `source`.
    ///
    /// Specifically, the buffer is cloned as well as the current buffer position and
    /// cursor values. All other attributes are initialized as if a new editor were
    /// being created.
    pub fn clone_as(&self, source: Source) -> Editor {
        let mut buffer = self.buffer().clone();
        buffer.set_pos(self.cur_pos);
        let mut editor = Self::new(self.config.clone(), source, Some(buffer));
        editor.cursor = self.cursor;
        editor
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    pub fn assume(&mut self, source: Source) {
        self.source = source;
    }

    #[inline]
    pub fn buffer(&self) -> Ref<'_, Buffer> {
        self.buffer.borrow()
    }

    #[inline]
    fn buffer_mut(&self) -> RefMut<'_, Buffer> {
        self.buffer.borrow_mut()
    }

    #[inline]
    fn tokenizer(&self) -> Ref<'_, Tokenizer> {
        self.tokenizer.borrow()
    }

    #[inline]
    fn tokenizer_mut(&self) -> RefMut<'_, Tokenizer> {
        self.tokenizer.borrow_mut()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
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

    /// Returns the size of the editor canvas.
    pub fn size(&self) -> Size {
        (self.rows, self.cols).into()
    }

    /// Returns the buffer position corresponding to the [`cursor`](Self::cursor).
    pub fn pos(&self) -> usize {
        self.cur_pos
    }

    /// Sets the cursor location and corresponding buffer position to `cursor`, though
    /// the final cursor location is constrained by end-of-line and end-of-buffer
    /// boundaries.
    ///
    /// This function was designed for responding to *mouse click* events where the
    /// position of the click is captured in `cursor`.
    ///
    /// The coordinates in `cursor` are presumed to be relative to the origin of the
    /// editor canvas.
    pub fn set_focus(&mut self, cursor: Point) {
        // Ensure target cursor is bounded by effective area of canvas, which takes
        // into account left margin if enabled.
        let try_row = cmp::min(cursor.row, self.rows);
        let try_col = if cursor.col < self.margin_cols {
            0
        } else {
            cmp::min(cursor.col - self.margin_cols, self.cols)
        };

        // Find effective cursor location and buffer position by moving down from
        // top line of display.
        self.cur_line = self.top_line.clone();
        let row = self.down_cur_line(try_row);
        let col = self.cur_line.snap_col(try_col, self.cols);
        self.snap_col = Some(col);
        self.cur_pos = self.cur_line.pos_of(col);
        self.cursor = Point::new(row, col);
    }

    /// Attaches the `window` to this editor.
    pub fn attach(&mut self, window: WindowRef, align: Align) {
        let is_zombie = window.borrow().is_zombie();
        self.canvas = window.borrow().canvas().clone();
        self.banner = window.borrow().banner().clone();

        // Allocate leftmost columns of window to line numbers, but only if enabled and
        // total width of window is large enough to reasonably accommodate.
        let Size { rows, cols } = self.canvas.borrow().size();
        self.margin_cols = if self.config.settings.lines && cols >= Self::MARGIN_COLS * 2 {
            Self::MARGIN_COLS
        } else {
            0
        };
        self.rows = rows;
        self.cols = cols - self.margin_cols;

        if !is_zombie {
            self.align_cursor(align);
            self.draw();
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
        self.align_syntax();
        self.cursor = Point::new(row, col);
    }

    fn align_syntax(&mut self) {
        self.syntax_cursor = self
            .tokenizer
            .borrow()
            .find(self.syntax_cursor, self.top_line.row_pos);
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
            .set_source(self.source.clone())
            .set_syntax(self.tokenizer().syntax().name.clone())
            .set_location(self.location())
            .draw();
    }

    /// Tries to move the cursor *backward* from the current buffer position by `len`
    /// characters.
    pub fn move_backward(&mut self, len: usize) {
        let pos = self.cur_pos - cmp::min(len, self.cur_pos);
        if pos < self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    /// Tries to move the cursor *forward* from the current buffer position by `len`
    /// characters.
    pub fn move_forward(&mut self, len: usize) {
        let pos = cmp::min(self.cur_pos + len, self.buffer().size());
        if pos > self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    /// Tries to move the cursor *backward* by one word from the current buffer
    /// position.
    pub fn move_backward_word(&mut self) {
        let pos = self.find_word_before(self.cur_pos);
        if pos < self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    /// Tries to move the cursor *forward* by one word from the current buffer
    /// position.
    pub fn move_forward_word(&mut self) {
        let pos = self.find_word_after(self.cur_pos);
        if pos > self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    /// Returns the position of the word that comes before `pos`.
    fn find_word_before(&self, pos: usize) -> usize {
        self.buffer()
            .backward(pos)
            .index()
            .skip_while(|(_, c)| c.is_whitespace())
            .skip_while(|(_, c)| !c.is_whitespace())
            .next()
            .map(|(pos, _)| pos + 1)
            .unwrap_or(0)
    }

    /// Returns the position of the word that follows after `pos`.
    fn find_word_after(&self, pos: usize) -> usize {
        self.buffer()
            .forward(pos)
            .index()
            .skip_while(|(_, c)| !c.is_whitespace())
            .skip_while(|(_, c)| c.is_whitespace())
            .next()
            .map(|(pos, _)| pos)
            .unwrap_or(self.buffer().size())
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
            let col = self.cur_line.snap_col(try_col, self.cols);
            self.cur_pos = self.cur_line.pos_of(col);
            self.align_syntax();
            self.cursor = Point::new(row, col);
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
            let col = self.cur_line.snap_col(try_col, self.cols);
            self.cur_pos = self.cur_line.pos_of(col);
            self.align_syntax();
            self.cursor = Point::new(row, col);
        }
    }

    /// Moves the cursor to the *start* of the current row.
    pub fn move_start(&mut self) {
        if self.cursor.col > 0 {
            self.cur_pos = self.cur_line.row_pos;
            self.cursor.col = 0;
        }
        self.snap_col = None;
    }

    /// Moves the cursor to the *end* of the current row.
    pub fn move_end(&mut self) {
        let end_col = self.cur_line.end_col(self.cols);
        if self.cursor.col < end_col {
            self.cur_pos = self.cur_line.pos_of(end_col);
            self.cursor.col = end_col;
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
        self.align_syntax();
        self.cursor = Point::new(row, col);
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
                let col = self.cur_line.snap_col(try_col, self.cols);
                self.cur_pos = self.cur_line.pos_of(col);
                (0, col)
            } else {
                // Cursor still visible on display.
                (self.cursor.row - rows, self.cursor.col)
            };
            self.align_syntax();
            self.cursor = Point::new(row, col);
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
                let col = self.cur_line.snap_col(try_col, self.cols);
                self.cur_pos = self.cur_line.pos_of(col);
                (self.rows - 1 as u32, col)
            };
            self.align_syntax();
            self.cursor = Point::new(row, col);
        }
    }

    /// Inserts the character `c` at the current buffer position.
    pub fn insert_char(&mut self, c: char) {
        self.insert_normal(&[c])
    }

    /// Inserts the string slice `str` at the current buffer position.
    pub fn insert_str(&mut self, text: &str) {
        self.insert_normal(&text.chars().collect::<Vec<_>>())
    }

    /// Inserts the array of `text` at the current buffer position.
    pub fn insert(&mut self, text: &[char]) {
        self.insert_normal(text);
    }

    fn insert_normal(&mut self, text: &[char]) {
        self.insert_internal(text, Some(Log::Normal));
    }

    /// An internal workhorse to which all *insertion* functions delegate.
    fn insert_internal(&mut self, text: &[char], log: Option<Log>) {
        if text.len() > 0 {
            // Most common use case is single-character insertions, so favor use of
            // more efficient buffer insertion in that case.
            self.buffer_mut().set_pos(self.cur_pos);
            let cur_pos = if text.len() == 1 {
                self.buffer_mut().insert_char(text[0])
            } else {
                self.buffer_mut().insert(text)
            };

            // Log change to buffer.
            if let Some(_) = log {
                self.log(Change::Insert(self.cur_pos, text.to_vec()));
            }

            // Update tokenizer with insertion range.
            let cursor = self.tokenizer().find(self.syntax_cursor, self.cur_pos);
            self.tokenizer_mut().insert(cursor, text.len());
            let cursor = self.tokenizer_mut().tokenize(&self.buffer());
            self.syntax_cursor = cursor;

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
            self.align_syntax();
            self.cursor = Point::new(row, col);
            self.dirty = true;
        }
    }

    /// Removes and returns the character before the current buffer position.
    ///
    /// An empty vector is returned if the current position is already at the top
    /// of the buffer.
    pub fn remove_before(&mut self) -> Vec<char> {
        if self.cur_pos > 0 {
            self.remove(self.cur_pos - 1)
        } else {
            vec![]
        }
    }

    /// Removes and returns the character after the current buffer position.
    ///
    /// An empty vector is returned if the current position is already at the
    /// bottom of the buffer.
    pub fn remove_after(&mut self) -> Vec<char> {
        if self.cur_pos < self.buffer().size() {
            self.remove(self.cur_pos + 1)
        } else {
            vec![]
        }
    }

    /// Removes and returns the text between the current buffer position and `mark`.
    pub fn remove_mark(&mut self, mark: Mark) -> Vec<char> {
        let Mark(pos, soft) = mark;
        self.remove_internal(pos, Some(Log::Selection(soft)))
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
            self.remove_before()
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
        self.remove_internal(pos, Some(Log::Normal))
    }

    /// An internal workhorse to which all *removal* functions delegate.
    fn remove_internal(&mut self, pos: usize, log: Option<Log>) -> Vec<char> {
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

            // Log change to buffer.
            if let Some(log) = log {
                match log {
                    Log::Normal => {
                        self.log(if pos < self.cur_pos {
                            Change::RemoveBefore(self.cur_pos, text.clone())
                        } else {
                            Change::RemoveAfter(self.cur_pos, text.clone())
                        });
                    }
                    Log::Selection(soft) => {
                        self.log(if pos < self.cur_pos {
                            Change::RemoveSelectionBefore(self.cur_pos, text.clone(), soft)
                        } else {
                            Change::RemoveSelectionAfter(self.cur_pos, text.clone(), soft)
                        });
                    }
                }
            }

            // Update tokenizer with removal range.
            let cursor = self.tokenizer().find(self.syntax_cursor, from_pos);
            self.tokenizer_mut().remove(cursor, text.len());
            let cursor = self.tokenizer_mut().tokenize(&self.buffer());
            self.syntax_cursor = cursor;

            // Removal of text requires current and top lines to be updated since may
            // have changed.
            self.cur_line = self.update_line(&self.cur_line);
            self.top_line = self.update_line(&self.top_line);
            self.cur_pos = from_pos;
            let col = self.cur_line.col_of(self.cur_pos);
            self.snap_col = None;
            self.align_syntax();
            self.cursor = Point::new(row, col);
            self.dirty = true;
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

    /// Sets a *soft* mark at buffer position `pos` unless a *soft* mark was previously
    /// set.
    ///
    /// Note that if a *hard* mark was previously set, the *soft* mark will replace
    /// it.
    ///
    /// Returns the previous *hard* mark if set, otherwise `None`.
    pub fn set_soft_mark_at(&mut self, pos: usize) -> Option<Mark> {
        let pos = cmp::min(pos, self.buffer().size());
        if let Some(mark @ Mark(_, soft)) = self.mark {
            if soft {
                None
            } else {
                self.mark = Some(Mark(pos, true));
                Some(mark)
            }
        } else {
            self.mark = Some(Mark(pos, true));
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

    /// Reverts the last change to the buffer, if any, and makes that change eligible
    /// to be reapplied via [`redo`](Editor::redo).
    ///
    /// Returns `true` if the change was reverted and `false` if the *undo* stack is
    /// empty.
    pub fn undo(&mut self) -> bool {
        if let Some(change) = self.undo.pop() {
            self.undo_change(&change);
            self.redo.push(change);
            true
        } else {
            false
        }
    }

    /// Applies the last change to the buffer, if any, that was reverted via
    /// [`undo`](Editor::undo).
    ///
    /// Returns `true` if the change was applies and `false` if the *redo* stack is
    /// empty.
    pub fn redo(&mut self) -> bool {
        if let Some(change) = self.redo.pop() {
            self.redo_change(&change);
            self.undo.push(change);
            true
        } else {
            false
        }
    }

    /// Reverts `change`.
    fn undo_change(&mut self, change: &Change) {
        match change {
            Change::Insert(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto);
                self.remove_internal(pos + text.len(), None);
            }
            Change::RemoveBefore(pos, text) => {
                self.clear_mark();
                self.move_to(pos - text.len(), Align::Auto);
                self.insert_internal(text, None);
            }
            Change::RemoveAfter(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto);
                self.insert_internal(text, None);
                self.move_to(*pos, Align::Auto);
            }
            Change::RemoveSelectionBefore(pos, text, soft) => {
                self.move_to(pos - text.len(), Align::Auto);
                if *soft {
                    self.set_soft_mark();
                } else {
                    self.set_hard_mark();
                }
                self.insert_internal(text, None);
            }
            Change::RemoveSelectionAfter(pos, text, soft) => {
                self.move_to(*pos, Align::Auto);
                self.insert_internal(text, None);
                if *soft {
                    self.set_soft_mark();
                } else {
                    self.set_hard_mark();
                }
                self.move_to(*pos, Align::Auto);
            }
        }
    }

    /// Applies `change`.
    fn redo_change(&mut self, change: &Change) {
        match change {
            Change::Insert(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto);
                self.insert_internal(text, None);
            }
            Change::RemoveBefore(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto);
                self.remove_internal(pos - text.len(), None);
            }
            Change::RemoveAfter(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto);
                self.remove_internal(pos + text.len(), None);
            }
            Change::RemoveSelectionBefore(pos, text, _) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto);
                self.remove_internal(pos - text.len(), None);
            }
            Change::RemoveSelectionAfter(pos, text, _) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto);
                self.remove_internal(pos + text.len(), None);
            }
        }
    }

    /// Logs `change` by pushing it onto the *undo* stack and clearing the *redo*
    /// stack.
    fn log(&mut self, change: Change) {
        const UNDO_SOFT_LIMIT: usize = 1024;
        const UNDO_HARD_LIMIT: usize = 1280;

        if let Some(top) = self.undo.pop() {
            if let Some(combined) = change.possibly_combine(&top) {
                self.undo.push(combined);
            } else {
                self.undo.push(top);
                self.undo.push(change);
            }
        } else {
            self.undo.push(change);
        }
        self.redo.clear();

        // Trim undo stack to soft limit once size exceeds hard limit, as this avoids
        // repeatedly trimming with every change.
        if self.undo.len() > UNDO_HARD_LIMIT {
            let n = self.undo.len() - UNDO_SOFT_LIMIT;
            self.undo.drain(0..n);
        }
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
        while pos >= self.cur_line.end_pos() && !self.cur_line.is_bottom(self.cols) {
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
        if line.is_bottom(self.cols) {
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
            .try_fold(render, |render, c| self.render_cell(&draw, render, c));
        if let Some(render) = rest {
            self.render_rest(&draw, render);
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
    fn render_cell(&self, draw: &Draw, render: Render, c: char) -> Option<Render> {
        self.render_margin(draw, &render);
        let mut canvas = self.canvas.borrow_mut();
        let (row, col) = (render.row, render.col + self.margin_cols);
        let render = if c == '\n' {
            canvas.set_cell(row, col, draw.as_text(c, &render));
            canvas.fill_cell_from(row, col + 1, draw.as_text(' ', &render));
            render.next_line()
        } else {
            canvas.set_cell(row, col, draw.as_text(c, &render));
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
    fn render_rest(&self, draw: &Draw, render: Render) {
        self.render_margin(draw, &render);
        let mut canvas = self.canvas.borrow_mut();

        // Blank out rest of existing row.
        let (row, col) = (render.row, render.col + self.margin_cols);
        canvas.fill_cell_from(row, col, draw.as_text(' ', &render));

        // Blank out remaining rows.
        for row in (render.row + 1)..self.rows {
            if self.margin_cols > 0 {
                canvas.fill_cell(row, 0..self.margin_cols, draw.as_margin(' '));
            }
            canvas.fill_cell_from(row, self.margin_cols, draw.as_blank());
        }
    }

    /// Renders the margin if line numbering is enabled and the rendering context is
    /// on the first column of any row.
    fn render_margin(&self, draw: &Draw, render: &Render) {
        if render.col == 0 && self.margin_cols > 0 {
            let mut canvas = self.canvas.borrow_mut();
            if render.line_wrapped {
                canvas.fill_cell(render.row, 0..self.margin_cols, draw.as_margin(' '));
            } else if render.line < Self::LINE_LIMIT {
                let s = format!(
                    "{:>cols$} ",
                    render.line,
                    cols = Self::MARGIN_COLS as usize - 1
                );
                for (col, c) in s.char_indices() {
                    canvas.set_cell(render.row, col as u32, draw.as_margin(c));
                }
            } else {
                canvas.fill_cell(render.row, 0..self.margin_cols - 1, draw.as_margin('-'));
                canvas.set_cell(render.row, self.margin_cols, draw.as_margin(' '));
            }
        }
    }
}
