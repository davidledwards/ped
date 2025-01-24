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

/// An editing session containing a [`kernel`](EditorKernel) that carries out most
/// operations.
pub struct Editor {
    /// The kernel where all operations are implemented.
    kernel: EditorKernel,

    /// A value of `true` implies that _mutable_ operations are not allowed.
    readonly: bool,
}

pub type EditorRef = Rc<RefCell<Editor>>;

/// A collection of _immutable_ operations that can be performed on an [`Editor`].
///
/// The notion of immutability in this context is not strictly defined, but rather
/// seen from the perspective of the kinds of operations that can be performed
/// regardless of whether the underlying buffer can be mutated. Hence, some of these
/// operations actually require mutable references even though they are logically
/// immutable.
pub trait ImmutableEditor {
    /// Returns a partial clone of this editor using `source`.
    ///
    /// Specifically, the buffer is cloned as well as the current buffer position and
    /// cursor values. All other attributes are initialized as if a new editor were
    /// being created.
    fn clone_as(&self, source: Source) -> Editor;

    /// Returns a reference to the source.
    fn source(&self) -> &Source;

    /// Changes the `source` associated with this editor.
    fn assume(&mut self, source: Source);

    /// Returns a reference to the underlying buffer.
    fn buffer(&self) -> Ref<'_, Buffer>;

    /// Returns `true` if the buffer has changed.
    fn is_dirty(&self) -> bool;

    /// Makes this editor _not_ dirty.
    fn clear_dirty(&mut self);

    /// Returns the cursor position on the display in terms of _row_ and _column_.
    ///
    /// The _row_ and _column_ values are `0`-based and exclusively bounded by
    /// [`size()`](Self::size).
    fn cursor(&self) -> Point;

    /// Returns the location of the cursor position in the buffer in terms of _line_
    /// and _column_.
    ///
    /// The _line_ and _column_ values are `0`-based. Note that neither of these values
    /// are bounded by the size of the display, which is the case with
    /// [`cursor`](Self::cursor).
    fn location(&self) -> Point;

    /// Returns the number of rows available on the editor canvas.
    fn rows(&self) -> u32;

    /// Returns the size of the editor canvas.
    fn size(&self) -> Size;

    /// Returns the buffer position corresponding to the [`cursor`](Self::cursor).
    fn pos(&self) -> usize;

    /// Sets the cursor location and corresponding buffer position to `cursor`, though
    /// the final cursor location is constrained by end-of-line and end-of-buffer
    /// boundaries.
    ///
    /// This function was designed for responding to _mouse click_ events where the
    /// position of the click is captured in `cursor`.
    ///
    /// The coordinates in `cursor` are presumed to be relative to the origin of the
    /// editor canvas.
    fn set_focus(&mut self, cursor: Point);

    /// Attaches the `window` to this editor.
    fn attach(&mut self, window: WindowRef, align: Align);

    /// Detaches the existing window from this editor.
    fn detach(&mut self);

    /// Sets the position of the cursor based on the alignment objective `align`.
    fn align_cursor(&mut self, align: Align);

    /// Draws the canvas and banner regardless of whether any updates have occurred.
    fn draw(&mut self);

    /// Makes the cursor visible.
    fn show_cursor(&mut self);

    /// Tries to move the cursor _backward_ from the current buffer position by `len`
    /// characters.
    fn move_backward(&mut self, len: usize);

    /// Tries to move the cursor _forward_ from the current buffer position by `len`
    /// characters.
    fn move_forward(&mut self, len: usize);

    /// Tries to move the cursor _backward_ by one word from the current buffer
    /// position.
    fn move_backward_word(&mut self);

    /// Tries to move the cursor _forward_ by one word from the current buffer
    /// position.
    fn move_forward_word(&mut self);

    /// Tries to move the cursor _up_ by the specified number of `try_rows`.
    ///
    /// If `pin` is `true`, then the cursor will remain on the current row if the
    /// resulting display makes it possible. Pinning is useful when _paging up_.
    ///
    /// If `pin` is `false`, then the cursor will move up in tandem with `try_rows`,
    /// though not to extend beyond the top of the display.
    fn move_up(&mut self, try_rows: u32, pin: bool);

    /// Tries to move the cursor _down_ by the specified number of `try_rows`.
    ///
    /// If `pin` is `true`, then the cursor will remain on the current row. Pinning is
    /// useful when _paging down_.
    ///
    /// If `pin` is `false`, then the cursor will move down in tandem with `try_rows`,
    /// though not to extend beyond the bottom of the display.
    fn move_down(&mut self, try_rows: u32, pin: bool);

    /// Moves the cursor to the _start_ of the current row.
    fn move_start(&mut self);

    /// Moves the cursor to the _end_ of the current row.
    fn move_end(&mut self);

    /// Moves the cursor to the _top_ of the buffer.
    fn move_top(&mut self);

    /// Moves the cursor to the _bottom_ of the buffer.
    fn move_bottom(&mut self);

    /// Moves the buffer position to the first character of `line` and places the
    /// cursor on the display according to the `align` objective.
    fn move_line(&mut self, line: u32, align: Align);

    /// Moves the current buffer position to `pos` and places the cursor on the
    /// display according to the `align` objective.
    ///
    /// When [`Align::Auto`] is specified, the placement of the cursor depends on
    /// the target `pos` relative to the current buffer position. Specifically, it
    /// behaves as follows:
    /// - _when `pos` is above the current line but still visible on the display_:
    ///   aligns the cursor on the target row above the current line, though not to
    ///   extend beyond the top row
    /// - _when `pos` is on the current line_: aligns the cursor on the current row
    /// - _when `pos` is beyond the current line_: aligns the cursor on the target
    ///   row below the current line, though not to extend beyond the borrom row
    fn move_to(&mut self, pos: usize, align: Align);

    /// Tries scrolling _up_ the contents of the display by the specified number of
    /// `try_rows` while preserving the cursor position, which also means the cursor
    /// moves _up_ as the contents scroll.
    fn scroll_up(&mut self, try_rows: u32);

    /// Tries scrolling _down_ the contents of the display by the specified number of
    /// `try_rows` while preserving the cursor position, which also means the cursor
    /// moves _down_ as the contents scroll.
    fn scroll_down(&mut self, try_rows: u32);

    /// Sets a _hard_ mark at the current buffer position and returns the previous
    /// mark if set.
    fn set_hard_mark(&mut self) -> Option<Mark>;

    /// Sets a _soft_ mark at the current buffer position unless a _soft_ mark was
    /// previously set.
    ///
    /// Note that if a _hard_ mark was previously set, the _soft_ mark will replace
    /// it.
    ///
    /// Returns the previous _hard_ mark if set, otherwise `None`.
    fn set_soft_mark(&mut self) -> Option<Mark>;

    /// Sets a _soft_ mark at buffer position `pos` unless a _soft_ mark was previously
    /// set.
    ///
    /// Note that if a _hard_ mark was previously set, the _soft_ mark will replace
    /// it.
    ///
    /// Returns the previous _hard_ mark if set, otherwise `None`.
    fn set_soft_mark_at(&mut self, pos: usize) -> Option<Mark>;

    /// Clears and returns the mark if _soft_, otherwise `None` is returned.
    fn clear_soft_mark(&mut self) -> Option<Mark>;

    /// Clears and returns the mark.
    fn clear_mark(&mut self) -> Option<Mark>;

    /// Returns the text between the current buffer position and `mark`.
    fn copy_mark(&self, mark: Mark) -> Vec<char>;

    /// Returns the text of the line on which the current buffer position rests.
    fn copy_line(&self) -> Vec<char>;

    /// Returns the text between `from_pos` and `end_pos`.
    ///
    /// Specifically, the range of characters is bounded _inclusively below_ and
    /// _exclusively above_. If `from_pos` is less than `to_pos`, then the range is
    /// [`from_pos`, `to_pos`), otherwise it is [`to_pos`, `from_pos`).
    ///
    /// This function will return an empty vector if `from_pos` is equal to `to_pos`.
    fn copy(&self, from_pos: usize, to_pos: usize) -> Vec<char>;

    /// Reverts the last change to the buffer, if any, and makes that change eligible
    /// to be reapplied via [`redo`](Editor::redo).
    ///
    /// Returns `true` if the change was reverted and `false` if the _undo_ stack is
    /// empty.
    fn undo(&mut self) -> bool;

    /// Applies the last change to the buffer, if any, that was reverted via
    /// [`undo`](Editor::undo).
    ///
    /// Returns `true` if the change was applies and `false` if the _redo_ stack is
    /// empty.
    fn redo(&mut self) -> bool;

    /// Returned the captured state of the editor.
    fn capture(&self) -> Capture;

    /// Restores the editor to the captured state in `capture`.
    ///
    /// Note that if the editor changes after state has been captured, there is no
    /// guarantee that said state will be restored precisely as it was.
    fn restore(&mut self, capture: &Capture);

    /// Tokenizes the buffer if changes occurred since the last tokenization, returning
    /// `true` if tokenization occurred and `false` otherwise.
    fn tokenize(&mut self) -> bool;

    /// Renders the contents of the editor.
    fn render(&mut self);
}

/// A collection of _mutable_ operations that can be performed on an [`Editor`].
///
/// The notion of mutability in this context simply implies the ability to change the
/// underlying buffer.
///
/// This trait also inherits immutable operations, as these operations are typically
/// interleaved with mutable operations.
pub trait MutableEditor: ImmutableEditor {
    /// Inserts the character `c` at the current buffer position.
    fn insert_char(&mut self, c: char);

    /// Inserts the string slice `str` at the current buffer position.
    fn insert_str(&mut self, text: &str);

    /// Inserts the array of `text` at the current buffer position.
    fn insert(&mut self, text: &[char]);

    /// Removes and returns the character before the current buffer position.
    ///
    /// An empty vector is returned if the current position is already at the top
    /// of the buffer.
    fn remove_before(&mut self) -> Vec<char>;

    /// Removes and returns the character after the current buffer position.
    ///
    /// An empty vector is returned if the current position is already at the
    /// bottom of the buffer.
    fn remove_after(&mut self) -> Vec<char>;

    /// Removes and returns the text between the current buffer position and `mark`.
    fn remove_mark(&mut self, mark: Mark) -> Vec<char>;

    /// Removes and returns the text of the line on which the current buffer position
    /// rests.
    fn remove_line(&mut self) -> Vec<char>;

    /// Removes and returns the text between the start of the current line and the
    /// current buffer position.
    fn remove_start(&mut self) -> Vec<char>;

    /// Removes and returns the text between the current buffer position and the end
    /// of the current line.
    fn remove_end(&mut self) -> Vec<char>;

    /// Removes and returns the text between the current buffer position and `pos`.
    ///
    /// Specifically, the range of characters is bounded _inclusively below_ and
    /// _exclusively above_. If `pos` is less than the current buffer position, then
    /// the range is [`pos`, `cur_pos`), otherwise it is [`cur_pos`, `pos`).
    ///
    /// This function will return an empty vector if `pos` is equal to `cur_pos`.
    fn remove(&mut self, pos: usize) -> Vec<char>;
}

/// An editing kernel with an underlying [`Buffer`] and an attachable [`Window`].
struct EditorKernel {
    /// Global configuration.
    config: ConfigurationRef,

    /// The source of the buffer.
    source: Source,

    /// Buffer containing the contents of this editor.
    buffer: BufferRef,

    /// A logical clock that increments with each change to the buffer.
    clock: u64,

    /// A stack containing changes to the buffer that can be _undone_.
    undo: Vec<Change>,

    /// A stack containing changes to the buffer that can be _redone_.
    redo: Vec<Change>,

    /// Tokenizes the buffer for syntax coloring.
    tokenizer: TokenizerRef,

    /// The value of [`clock`](Self::clock) at the time of the last tokenization.
    tokenize_clock: u64,

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

    /// An optional column to which the cursor should _snap_ when moving up and down.
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

/// The distinct types of changes to a buffer recorded in the _undo_ and _redo_ stacks.
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
    /// value is `true` if it was a _soft_ mark and `false` if a _hard_ mark.
    Selection(bool),
}

/// Represents contextual information for a line on the display.
///
/// A _line_ in this context should not be confused with the characterization of
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
/// mark is _soft_, and `false` if _hard_.
#[derive(Copy, Clone)]
pub struct Mark(pub usize, pub bool);

/// A means of capturing the visual state of an editor for the purpose of possible
/// restoration.
pub struct Capture {
    pub pos: usize,
    pub cursor: Point,
    pub mark: Option<Mark>,
}

/// A drawing context provided to rendering functions.
struct Draw {
    /// Configuration that dictates colors and behaviors.
    config: ConfigurationRef,

    /// Color of margin.
    margin_color: Color,

    /// Color of text with no special treatment.
    text_color: Color,

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
    // Special character shown for \n (newline) when visible.
    const EOL_CHAR: char = '\u{21b2}';

    // Special character shown for \t (tab).
    const TAB_CHAR: char = '\u{2192}';

    // Special character shown for all other ASCII control characters.
    const CTRL_CHAR: char = '\u{00bf}';

    fn new(editor: &EditorKernel) -> Draw {
        let config = editor.config.clone();
        let margin_color = Color::new(config.theme.margin_fg, config.theme.margin_bg);
        let text_color = Color::new(config.theme.text_fg, config.theme.text_bg);

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
            config,
            margin_color,
            text_color,
            cursor: editor.cursor(),
            select_span,
        }
    }

    /// Formats `c` using the margin color.
    #[inline]
    fn as_margin(&self, c: char) -> Cell {
        Cell::new(c, self.margin_color)
    }

    /// Formats ` ` (space) using the text color.
    #[inline]
    fn as_blank(&self) -> Cell {
        Cell::new(' ', self.text_color)
    }

    /// Formats `c` using a color depending on the current rendering context.
    fn as_text(&self, c: char, render: &Render) -> Cell {
        let fg = if let Some(fg) = render.syntax_cursor.color() {
            fg
        } else {
            if (c == '\n' && self.config.settings.eol) || c.is_ascii_control() {
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

    /// Possibly converts `c` to an alternate display character.
    #[inline]
    fn convert_char(&self, c: char) -> char {
        match c {
            '\n' => {
                if self.config.settings.eol {
                    Self::EOL_CHAR
                } else {
                    ' '
                }
            }
            '\t' => Self::TAB_CHAR,
            c if c.is_ascii_control() => Self::CTRL_CHAR,
            c => c,
        }
    }
}

impl Render {
    /// Creates an initial rendering context from `editor`.
    fn new(editor: &EditorKernel) -> Render {
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
    /// Creates a readonly editor using `source` and `buffer`.
    ///
    /// A readonly editor is not permitted to obtain a mutable interface.
    pub fn readonly(config: ConfigurationRef, source: Source, buffer: Buffer) -> Editor {
        Self::new(config, source, Some(buffer), true)
    }

    /// Creates a mutable editor using `source` and an optional `buffer`, which if
    /// `None` automatically creates an empty buffer.
    ///
    /// A mutable editor is permitted to obtain a mutable interface.
    pub fn mutable(config: ConfigurationRef, source: Source, buffer: Option<Buffer>) -> Editor {
        Self::new(config, source, buffer, false)
    }

    fn new(
        config: ConfigurationRef,
        source: Source,
        buffer: Option<Buffer>,
        readonly: bool,
    ) -> Editor {
        Editor {
            kernel: EditorKernel::new(config, source, buffer),
            readonly,
        }
    }

    /// Turns the editor into a [`EditorRef`].
    pub fn to_ref(self) -> EditorRef {
        Rc::new(RefCell::new(self))
    }

    /// Returns a mutable editor if not classified as _readonly_, otherwise `None`.
    pub fn modify(&mut self) -> Option<&mut dyn MutableEditor> {
        if self.readonly {
            None
        } else {
            Some(&mut self.kernel)
        }
    }
}

impl ImmutableEditor for Editor {
    #[inline]
    fn clone_as(&self, source: Source) -> Editor {
        self.kernel.clone_as(source)
    }

    #[inline]
    fn source(&self) -> &Source {
        self.kernel.source()
    }

    #[inline]
    fn assume(&mut self, source: Source) {
        self.kernel.assume(source);
    }

    #[inline]
    fn buffer(&self) -> Ref<'_, Buffer> {
        self.kernel.buffer()
    }

    #[inline]
    fn is_dirty(&self) -> bool {
        self.kernel.is_dirty()
    }

    #[inline]
    fn clear_dirty(&mut self) {
        self.kernel.clear_dirty();
    }

    #[inline]
    fn cursor(&self) -> Point {
        self.kernel.cursor()
    }

    #[inline]
    fn location(&self) -> Point {
        self.kernel.location()
    }

    #[inline(always)]
    fn rows(&self) -> u32 {
        self.kernel.rows()
    }

    #[inline]
    fn size(&self) -> Size {
        self.kernel.size()
    }

    #[inline]
    fn pos(&self) -> usize {
        self.kernel.pos()
    }

    #[inline]
    fn set_focus(&mut self, cursor: Point) {
        self.kernel.set_focus(cursor);
    }

    #[inline]
    fn attach(&mut self, window: WindowRef, align: Align) {
        self.kernel.attach(window, align);
    }

    #[inline]
    fn detach(&mut self) {
        self.kernel.detach();
    }

    #[inline]
    fn align_cursor(&mut self, align: Align) {
        self.kernel.align_cursor(align);
    }

    #[inline]
    fn draw(&mut self) {
        self.kernel.draw();
    }

    #[inline]
    fn show_cursor(&mut self) {
        self.kernel.show_cursor();
    }

    #[inline]
    fn move_backward(&mut self, len: usize) {
        self.kernel.move_backward(len);
    }

    #[inline]
    fn move_forward(&mut self, len: usize) {
        self.kernel.move_forward(len);
    }

    #[inline]
    fn move_backward_word(&mut self) {
        self.kernel.move_backward_word();
    }

    #[inline]
    fn move_forward_word(&mut self) {
        self.kernel.move_forward_word();
    }

    #[inline]
    fn move_up(&mut self, try_rows: u32, pin: bool) {
        self.kernel.move_up(try_rows, pin);
    }

    #[inline]
    fn move_down(&mut self, try_rows: u32, pin: bool) {
        self.kernel.move_down(try_rows, pin);
    }

    #[inline]
    fn move_start(&mut self) {
        self.kernel.move_start();
    }

    #[inline]
    fn move_end(&mut self) {
        self.kernel.move_end();
    }

    #[inline]
    fn move_top(&mut self) {
        self.kernel.move_top();
    }

    #[inline]
    fn move_bottom(&mut self) {
        self.kernel.move_bottom();
    }

    #[inline]
    fn move_line(&mut self, line: u32, align: Align) {
        self.kernel.move_line(line, align);
    }

    #[inline]
    fn move_to(&mut self, pos: usize, align: Align) {
        self.kernel.move_to(pos, align);
    }

    #[inline]
    fn scroll_up(&mut self, try_rows: u32) {
        self.kernel.scroll_up(try_rows);
    }

    #[inline]
    fn scroll_down(&mut self, try_rows: u32) {
        self.kernel.scroll_down(try_rows);
    }

    #[inline]
    fn set_hard_mark(&mut self) -> Option<Mark> {
        self.kernel.set_hard_mark()
    }

    #[inline]
    fn set_soft_mark(&mut self) -> Option<Mark> {
        self.kernel.set_soft_mark()
    }

    #[inline]
    fn set_soft_mark_at(&mut self, pos: usize) -> Option<Mark> {
        self.kernel.set_soft_mark_at(pos)
    }

    #[inline]
    fn clear_soft_mark(&mut self) -> Option<Mark> {
        self.kernel.clear_soft_mark()
    }

    #[inline]
    fn clear_mark(&mut self) -> Option<Mark> {
        self.kernel.clear_mark()
    }

    #[inline]
    fn copy_mark(&self, mark: Mark) -> Vec<char> {
        self.kernel.copy_mark(mark)
    }

    #[inline]
    fn copy_line(&self) -> Vec<char> {
        self.kernel.copy_line()
    }

    #[inline]
    fn copy(&self, from_pos: usize, to_pos: usize) -> Vec<char> {
        self.kernel.copy(from_pos, to_pos)
    }

    #[inline]
    fn undo(&mut self) -> bool {
        self.kernel.undo()
    }

    #[inline]
    fn redo(&mut self) -> bool {
        self.kernel.redo()
    }

    #[inline]
    fn capture(&self) -> Capture {
        self.kernel.capture()
    }

    #[inline]
    fn restore(&mut self, capture: &Capture) {
        self.kernel.restore(capture);
    }

    #[inline]
    fn tokenize(&mut self) -> bool {
        self.kernel.tokenize()
    }

    #[inline]
    fn render(&mut self) {
        self.kernel.render();
    }
}

impl ImmutableEditor for EditorKernel {
    fn clone_as(&self, source: Source) -> Editor {
        Editor {
            kernel: self.clone_kernel(source),
            readonly: false,
        }
    }

    fn source(&self) -> &Source {
        &self.source
    }

    fn assume(&mut self, source: Source) {
        self.source = source;
    }

    #[inline]
    fn buffer(&self) -> Ref<'_, Buffer> {
        self.buffer.borrow()
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn clear_dirty(&mut self) {
        self.dirty = false;
        self.show_banner();
    }

    #[inline]
    fn cursor(&self) -> Point {
        self.cursor
    }

    #[inline]
    fn location(&self) -> Point {
        Point::new(self.cur_line.line, self.cur_line.line_col(self.cursor.col))
    }

    fn rows(&self) -> u32 {
        self.rows
    }

    fn size(&self) -> Size {
        (self.rows, self.cols).into()
    }

    fn pos(&self) -> usize {
        self.cur_pos
    }

    fn set_focus(&mut self, cursor: Point) {
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

    fn attach(&mut self, window: WindowRef, align: Align) {
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

    fn detach(&mut self) {
        self.attach(Window::zombie().to_ref(), Align::Auto);
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
        self.cur_line = self.find_line(self.cur_pos);
        let row = self.set_top_line(try_row);
        let col = self.cur_line.col_of(self.cur_pos);
        self.snap_col = None;
        self.align_syntax();
        self.cursor = Point::new(row, col);
    }

    fn draw(&mut self) {
        self.canvas.borrow_mut().clear();
        self.show_banner();
        self.render();
    }

    fn show_cursor(&mut self) {
        let cursor = if self.margin_cols > 0 {
            self.cursor + Size::cols(self.margin_cols)
        } else {
            self.cursor
        };
        self.canvas.borrow_mut().set_cursor(cursor);
    }

    fn move_backward(&mut self, len: usize) {
        let pos = self.cur_pos - cmp::min(len, self.cur_pos);
        if pos < self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    fn move_forward(&mut self, len: usize) {
        let pos = cmp::min(self.cur_pos + len, self.buffer().size());
        if pos > self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    fn move_backward_word(&mut self) {
        let pos = self.find_word_before(self.cur_pos);
        if pos < self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    fn move_forward_word(&mut self) {
        let pos = self.find_word_after(self.cur_pos);
        if pos > self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    fn move_up(&mut self, try_rows: u32, pin: bool) {
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

    fn move_down(&mut self, try_rows: u32, pin: bool) {
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

    fn move_start(&mut self) {
        if self.cursor.col > 0 {
            self.cur_pos = self.cur_line.row_pos;
            self.cursor.col = 0;
        }
        self.snap_col = None;
    }

    fn move_end(&mut self) {
        let end_col = self.cur_line.end_col(self.cols);
        if self.cursor.col < end_col {
            self.cur_pos = self.cur_line.pos_of(end_col);
            self.cursor.col = end_col;
        }
        self.snap_col = None;
    }

    fn move_top(&mut self) {
        self.move_to(0, Align::Top);
    }

    fn move_bottom(&mut self) {
        let pos = self.buffer().size();
        self.move_to(pos, Align::Bottom);
    }

    fn move_line(&mut self, line: u32, align: Align) {
        let pos = self.buffer().find_line(line);
        self.move_to(pos, align);
    }

    fn move_to(&mut self, pos: usize, align: Align) {
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

    fn scroll_up(&mut self, try_rows: u32) {
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

    fn scroll_down(&mut self, try_rows: u32) {
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

    fn set_hard_mark(&mut self) -> Option<Mark> {
        self.mark.replace(Mark(self.cur_pos, false))
    }

    fn set_soft_mark(&mut self) -> Option<Mark> {
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

    fn set_soft_mark_at(&mut self, pos: usize) -> Option<Mark> {
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

    fn clear_soft_mark(&mut self) -> Option<Mark> {
        if let Some(Mark(_, true)) = self.mark {
            self.clear_mark()
        } else {
            None
        }
    }

    fn clear_mark(&mut self) -> Option<Mark> {
        self.mark.take()
    }

    fn copy_mark(&self, mark: Mark) -> Vec<char> {
        let Range { start, end } = self.get_mark_range(mark);
        self.copy(start, end)
    }

    fn copy_line(&self) -> Vec<char> {
        let Range { start, end } = self.cur_line.line_range();
        self.copy(start, end)
    }

    fn copy(&self, from_pos: usize, to_pos: usize) -> Vec<char> {
        self.buffer().copy(from_pos, to_pos)
    }

    fn undo(&mut self) -> bool {
        if let Some(change) = self.undo.pop() {
            self.undo_change(&change);
            self.redo.push(change);
            true
        } else {
            false
        }
    }

    fn redo(&mut self) -> bool {
        if let Some(change) = self.redo.pop() {
            self.redo_change(&change);
            self.undo.push(change);
            true
        } else {
            false
        }
    }

    fn capture(&self) -> Capture {
        Capture {
            pos: self.cur_pos,
            cursor: self.cursor,
            mark: self.mark.clone(),
        }
    }

    fn restore(&mut self, capture: &Capture) {
        self.move_to(capture.pos, Align::Row(capture.cursor.row));
        if let Some(Mark(pos, soft)) = capture.mark {
            let pos = cmp::min(pos, self.buffer().size());
            self.mark = Some(Mark(pos, soft));
        } else {
            self.mark = None;
        }
    }

    fn tokenize(&mut self) -> bool {
        if self.tokenize_clock < self.clock {
            self.syntax_cursor = {
                let cursor = self.tokenizer_mut().tokenize(&self.buffer());
                cursor
            };
            self.align_syntax();
            self.tokenize_clock = self.clock;
            true
        } else {
            false
        }
    }

    fn render(&mut self) {
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
}

impl MutableEditor for EditorKernel {
    fn insert_char(&mut self, c: char) {
        self.insert_normal(&[c])
    }

    fn insert_str(&mut self, text: &str) {
        self.insert_normal(&text.chars().collect::<Vec<_>>())
    }

    fn insert(&mut self, text: &[char]) {
        self.insert_normal(text);
    }

    fn remove_before(&mut self) -> Vec<char> {
        if self.cur_pos > 0 {
            self.remove(self.cur_pos - 1)
        } else {
            vec![]
        }
    }

    fn remove_after(&mut self) -> Vec<char> {
        if self.cur_pos < self.buffer().size() {
            self.remove(self.cur_pos + 1)
        } else {
            vec![]
        }
    }

    fn remove_mark(&mut self, mark: Mark) -> Vec<char> {
        let Mark(pos, soft) = mark;
        self.remove_internal(pos, Some(Log::Selection(soft)))
    }

    fn remove_line(&mut self) -> Vec<char> {
        let Range { start, end } = self.cur_line.line_range();
        self.move_to(start, Align::Auto);
        self.remove(end)
    }

    fn remove_start(&mut self) -> Vec<char> {
        if self.cur_pos == self.cur_line.line_pos {
            self.remove_before()
        } else {
            self.remove(self.cur_line.line_pos)
        }
    }

    fn remove_end(&mut self) -> Vec<char> {
        self.remove(self.cur_line.line_pos + self.cur_line.line_len)
    }

    fn remove(&mut self, pos: usize) -> Vec<char> {
        self.remove_internal(pos, Some(Log::Normal))
    }
}

impl EditorKernel {
    /// Number of columns allocated to the margin.
    const MARGIN_COLS: u32 = 6;

    /// Exclusive upper bound on line numbers that can be displayed in the margin.
    const LINE_LIMIT: u32 = 10_u32.pow(Self::MARGIN_COLS - 1);

    /// Creates a new editor using `source` and an optional `buffer`, which if `None`
    /// automatically creates an empty buffer.
    fn new(config: ConfigurationRef, source: Source, buffer: Option<Buffer>) -> EditorKernel {
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
        } else if let Source::Ephemeral(_) = &source {
            config
                .registry
                .find(source.to_string())
                .map(|syntax| syntax.clone())
                .unwrap_or_else(|| Syntax::default())
        } else {
            Syntax::default()
        };

        // Tokenize buffer.
        let mut tokenizer = Tokenizer::new(syntax);
        let syntax_cursor = tokenizer.tokenize(&buffer.borrow());

        EditorKernel {
            config,
            source,
            buffer,
            clock: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            tokenizer: tokenizer.to_ref(),
            tokenize_clock: 0,
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

    /// Returns a partial clone of this kernel using `source`.
    fn clone_kernel(&self, source: Source) -> EditorKernel {
        let mut buffer = self.buffer().clone();
        buffer.set_pos(self.cur_pos);
        let mut editor = Self::new(self.config.clone(), source, Some(buffer));
        editor.cursor = self.cursor;
        editor
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

    /// Aligns the syntax cursor with the top line.
    fn align_syntax(&mut self) {
        self.syntax_cursor = self
            .tokenizer
            .borrow()
            .find(self.syntax_cursor, self.top_line.row_pos);
    }

    /// Sets the values of all banner attributes and draws it.
    fn show_banner(&mut self) {
        self.banner
            .borrow_mut()
            .set_dirty(self.dirty)
            .set_source(self.source.clone())
            .set_syntax(self.tokenizer().syntax().name.clone())
            .set_location(self.location())
            .draw();
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

    /// Inserts `text` such that the change is recorded in the undo stack.
    fn insert_normal(&mut self, text: &[char]) {
        self.insert_internal(text, Some(Log::Normal));
    }

    /// An internal workhorse to which all _insertion_ functions delegate.
    ///
    /// A `log` value of `None` indicates that the change is not recorded in the undo
    /// stack.
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
            self.syntax_cursor = {
                let mut tokenizer = self.tokenizer_mut();
                let cursor = tokenizer.find(self.syntax_cursor, self.cur_pos);
                tokenizer.insert(cursor, text.len())
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
            self.align_syntax();
            self.cursor = Point::new(row, col);
            self.dirty = true;
            self.clock += 1;
        }
    }

    /// An internal workhorse to which all _removal_ functions delegate.
    ///
    /// A `log` value of `None` indicates that the change is not recorded in the undo
    /// stack.
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
            self.syntax_cursor = {
                let mut tokenizer = self.tokenizer_mut();
                let cursor = tokenizer.find(self.syntax_cursor, from_pos);
                tokenizer.remove(cursor, text.len())
            };

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
            self.clock += 1;
            text
        }
    }

    fn get_mark_range(&self, mark: Mark) -> Range<usize> {
        let Mark(pos, _) = mark;
        if pos < self.cur_pos {
            pos..self.cur_pos
        } else {
            self.cur_pos..pos
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

    /// Logs `change` by pushing it onto the _undo_ stack and clearing the _redo_
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
        let (line_pos, next_pos, line_bottom) = self.find_line_bounds(pos);
        let line_len = next_pos - line_pos;
        let row_pos = pos - ((pos - line_pos) % self.cols as usize);
        let row_len = cmp::min(line_len - (row_pos - line_pos), self.cols as usize);
        Line {
            row_pos,
            row_len,
            line_pos,
            line_len,
            line: self.buffer().line_of(line_pos),
            line_bottom,
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
        let (next_pos, line_bottom) = self.buffer().find_next_line(line.line_pos);
        let line_len = next_pos - line.line_pos;
        let row_len = cmp::min(
            line_len - (line.row_pos - line.line_pos),
            self.cols as usize,
        );
        Line {
            row_len,
            line_len,
            line_bottom,
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
            let (line_pos, next_pos, line_bottom) = self.find_line_bounds(pos);
            let line_len = next_pos - line_pos;
            let row_pos = pos - ((pos - line_pos) % self.cols as usize);
            let row_len = cmp::min(line_len - (row_pos - line_pos), self.cols as usize);
            let l = Line {
                row_pos,
                row_len,
                line_pos,
                line_len,
                line: line.line - 1,
                line_bottom,
            };
            Some(l)
        }
    }

    /// An unchecked version of [prev_line](EditorKernel::prev_line) that assumes `line`
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
            let (next_pos, line_bottom) = self.buffer().find_next_line(line_pos);
            let line_len = next_pos - line_pos;
            let row_len = cmp::min(line_len, self.cols as usize);
            let l = Line {
                row_pos: line_pos,
                row_len,
                line_pos,
                line_len,
                line: line.line + 1,
                line_bottom,
            };
            Some(l)
        }
    }

    /// An unchecked version of [next_line](EditorKernel::next_line) that assumes `line`
    /// is not at the bottom of the buffer.
    fn next_line_unchecked(&self, line: &Line) -> Line {
        self.next_line(line)
            .unwrap_or_else(|| panic!("line already at bottom of buffer"))
    }

    /// Returns a tuple, relative to the buffer line corresponding to `pos`, containing
    /// the position of the first character on that line, the position of the first
    /// character of the next line, and a boolean value indicating if the end of buffer
    /// has been reached.
    fn find_line_bounds(&self, pos: usize) -> (usize, usize, bool) {
        let buffer = self.buffer.borrow();
        let line_pos = buffer.find_start_line(pos);
        let (next_pos, line_bottom) = buffer.find_next_line(pos);
        (line_pos, next_pos, line_bottom)
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
