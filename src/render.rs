//! A specification for text rendering engines.
//!
//! Every rendering engine is required to implement the [Renderer] trait, which allows
//! the editor to interact with a consistent interface.
//!
//! There are currently two rendering engines available to the editor:
//! - a _wrapping_ engine that wraps long lines, and
//! - a _scrolling_ engine that does not wrap long lines.
//!
//! Both engines share a common strategy that optimizes for a minimal amount of overhead
//! in managing state information in exchange for a slightly less efficient use of
//! computational resources. In practice, the additional computing overhead is negligible
//! due to the efficiency of the algorithm.
//!
//! The rendering engine keeps two key pieces of information that allows it to
//! efficiently move around the buffer:
//! - a reference to the _top_ row of the display, and
//! - a reference to the _current_ row, which is where the cursor appears.
//!
//! THe distinction between a row and a line of text is important. A line of text,
//! as one might expect, is a sequence of characters in a buffer ending with `\n`. A
//! row represents a single line on the display, which may or may not contain an
//! entire line of text from the buffer. That depends on the type of rendering engine.
//!
//! The following illustrates how the _top_ and _current_ references point to specific
//! areas of the display. Both are instances of a `Row` struct that contains similar
//! but different information based on the type of rendering engine.
//!
//! ```text
//!            +-----------------------------------------------+
//!     top -> |fn main() -> ExitCode {                        |
//!            |   match run() {                               |
//!            |      Err(e) => {                              |
//! current -> |         let _ = writeln!(stderr(), "{e}");    |
//!            |         ExitCode::from(1)                     |
//!            |      }                                        |
//!            +-----------------------------------------------+
//! ```
//!
//! As the user moves the cursor around the display, _top_ and _current_ rows change
//! accordingly. For example, if the cursor is somewhere in the middle of the display
//! and moves up, the _current_ row reference will point to the prior row but the _top_
//! row stays the same. However, if the cursor is at the top or bottom row and the
//! cursor moves up or down, the _top_ row reference will also change accordingly
//! because the entire contents of the display shifts up or down.
//!
//! Moving a row reference up or down requires scanning the text buffer forward or
//! backward depending on the direction of the move. Most operations only move a short
//! distance, so the cost of scanning is negligible.
//!
//! The rendering algorithm starts at the _top_ row, which contains the buffer position
//! of the top-left cell in the display, moving left to right and top to bottom, writing
//! each character to the canvas.

mod scrolling;
mod wrapping;

use crate::buffer::{Buffer, BufferRef};
use crate::config::ConfigurationRef;
use crate::nav::{self, Location};
use crate::render::scrolling::ScrollingRenderer;
use crate::render::wrapping::WrappingRenderer;
use crate::size::{Point, Size};
use crate::style::Style;
use crate::token::{Position, Tokenizer};
use crate::window::WindowRef;

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

    /// Returns the cursor position on the display in terms of _row_ and _column_ if
    /// visible, otherwise `None`.
    ///
    /// The _row_ and _column_ values are `0`-based and exclusively bounded by
    /// [`size()`](Self::size).
    fn cursor(&self) -> Option<Point>;

    /// Returns the location of the cursor position in the buffer.
    fn location(&self) -> Location;

    /// Returns the starting (_inclusive_) and ending (_exclusive_) buffer positions
    /// of the line occupied by the cursor.
    fn line(&self) -> (usize, usize);

    /// Returns the starting buffer position of the top row of the display.
    fn origin(&self) -> usize;

    /// Sets the cursor location and corresponding buffer position to `cursor`,
    /// returning the actual cursor position which may be constrained by boundary
    /// restrictions.
    ///
    /// This function was designed for responding to _mouse click_ events where the
    /// position of the click is captured in `cursor`.
    ///
    /// The coordinates in `cursor` are presumed to be relative to the origin of the
    /// editor canvas.
    fn focus_cursor(&mut self, cursor: Point) -> Point;

    /// Sets the position of the cursor based on the `align` and `justify` objectives,
    /// returning the actual cursor position which may be constrained by boundary
    /// restrictions.
    fn align_cursor(&mut self, align: Align, justify: Justify) -> Point;

    /// Either _shows_ or _hides_ the cursor depending on the visbility of the cursor.
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
    /// display according to the `align` and `justify` objectives.
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
    fn move_to(&mut self, pos: usize, align: Align, justify: Justify);

    /// Tries scrolling the contents of the display in an _upward_ direction by the
    /// specified number of `try_rows` while also preserving the cursor position,
    /// returning the actual number of rows scrolled.
    ///
    /// Conceptually, this function moves the viewable area towards the top of the
    /// buffer.
    fn scroll_up(&mut self, try_rows: u32) -> u32;

    /// Tries scrolling the contents of the display in a _downward_ direction by the
    /// specified number of `try_rows` while also preserving the cursor position,
    /// returning the actual number of rows scrolled.
    ///
    /// Conceptually, this function moves the viewable area towards the bottom of the
    /// buffer.
    fn scroll_down(&mut self, try_rows: u32) -> u32;

    /// Tries scrolling the contents of the display in a _leftward_ direction by the
    /// specified number of `try_cols` while also preserving the cursor position,
    /// returning the actual number of columns scrolled.
    ///
    /// Conceptually, this function moves the viewable area towards the rightmost column
    /// of the current row.
    fn scroll_left(&mut self, try_cols: u32) -> u32;

    /// Tries scrolling the contents of the display in a _rightward_ direction by the
    /// specified number of `try_cols` while also preserving the cursor position,
    /// returning the actual number of columns scrolled.
    ///
    /// Conceptually, this function moves the viewable area towards the leftmost column
    /// of the current row.
    fn scroll_right(&mut self, try_cols: u32) -> u32;

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
    fn render(&mut self, tokenizer: &Tokenizer, token_pos: Position, style: &Style);
}

/// The types of rendering engines.
pub enum Rendering {
    /// A rendering engine that wraps lines exceeding the width of the display.
    Wrapping,

    /// A rendering engine the scrolls text horizontally for lines exceeding the width of
    /// the display.
    Scrolling,
}

/// Cursor _row_ alignment directives.
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

/// Cursor _column_ justification objective.
pub enum Justify {
    /// Try justifying the cursor based on its contextual use.
    Auto,

    /// Try justifying the cursor in the center of the row.
    Center,

    /// Try justifying the cursor at the left margin.
    Left,

    /// Try justifying the cursor at the right margin.
    Right,

    /// Try justifying the cursor at the specified column.
    Col(u32),
}

impl Rendering {
    /// Create a new rendering engine.
    pub fn create(&self, config: ConfigurationRef, buffer: BufferRef) -> Box<dyn Renderer> {
        match self {
            Self::Wrapping => Box::new(WrappingRenderer::new(config, buffer)),
            Self::Scrolling => Box::new(ScrollingRenderer::new(config, buffer)),
        }
    }
}

/// Provides common functions across _row_ implementations.
trait Rowable: Copy + Clone {
    /// Returns the buffer position at the start of the row.
    fn start_pos(&self) -> usize;

    /// Returns the buffer position at the end of the row.
    fn end_pos(&self) -> usize;

    /// Returns `true` if the row is the bottom-most line in the buffer.
    fn is_bottom(&self) -> bool;

    /// Returns the starting (_inclusive_) and ending (_exclusive_) buffer positions
    /// of the line occupied by the cursor.
    fn line(&self) -> (usize, usize);

    /// Returns the adjusted length of the row.
    ///
    /// If the row is terminated by `\n`, then the adjusted length must be one less
    /// than the actual length.
    ///
    /// If the row is not terminated by `\n`, which only occurs at the
    /// [bottom](Self::is_bottom), then the adjusted length must be equal to the
    /// actual length.
    fn adjusted_len(&self) -> usize;

    /// Returns the row preceding this row, or `None` if this row is already at the
    /// top of the buffer.
    fn prev(&self, buffer: &Buffer) -> Option<Self>
    where
        Self: Sized;

    /// Returns the row following this row, or `None` if this row is already at the
    /// bottom of the buffer.
    fn next(&self, buffer: &Buffer) -> Option<Self>
    where
        Self: Sized;

    /// Moves _up_ by `try_rows` relative to this row, or possibly fewer rows if the
    /// top of the buffer is reached, returning a pair containing the resulting row
    /// and the actual number of rows moved.
    fn up(&self, buffer: &Buffer, try_rows: u32) -> (Self, u32)
    where
        Self: Sized,
    {
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
    fn down(&self, buffer: &Buffer, try_rows: u32) -> (Self, u32)
    where
        Self: Sized,
    {
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
    fn find_up(&self, buffer: &Buffer, pos: usize) -> (Self, u32)
    where
        Self: Sized,
    {
        let mut row = *self;
        let mut rows = 0;
        while pos < row.start_pos() {
            row = row.prev_unchecked(buffer);
            rows += 1;
        }
        (row, rows)
    }

    /// Finds the row _down_ from this row that contains `pos`, returning a pair
    /// containing the resulting row and the total number of rows moved.
    fn find_down(&self, buffer: &Buffer, pos: usize) -> (Self, u32)
    where
        Self: Sized,
    {
        let mut row = *self;
        let mut rows = 0;
        while pos >= row.end_pos() && !row.is_bottom() {
            row = row.next_unchecked(buffer);
            rows += 1;
        }
        (row, rows)
    }

    /// Finds the row _down_ from this row that contains `pos`, but scanning no more
    /// than `n` rows before the operation is terminated.
    ///
    /// Returns a pair containing the resulting row and the total number of rows moved,
    /// or `None` if the operation is terminated.
    fn findn_down(&self, buffer: &Buffer, pos: usize, n: u32) -> Option<(Self, u32)>
    where
        Self: Sized,
    {
        let mut row = *self;
        let mut rows = 0;
        while rows < n && pos >= row.end_pos() && !row.is_bottom() {
            row = row.next_unchecked(buffer);
            rows += 1;
        }
        if rows < n { Some((row, rows)) } else { None }
    }

    /// An unchecked version of [`prev`](Self::prev) that assumes this row is not at the
    /// top of the buffer.
    fn prev_unchecked(&self, buffer: &Buffer) -> Self
    where
        Self: Sized,
    {
        self.prev(buffer)
            .unwrap_or_else(|| panic!("row already at top of buffer"))
    }

    /// An unchecked version of [`next`](Self::next) that assumes this row is not at the
    /// top of the buffer.
    fn next_unchecked(&self, buffer: &Buffer) -> Self
    where
        Self: Sized,
    {
        self.next(buffer)
            .unwrap_or_else(|| panic!("row already at bottom of buffer"))
    }
}

/// Returns a tuple, relative to the line in `buffer` corresponding to `pos`,
/// containing the position of the first character on that line, the position of
/// the first character of the next line, and a boolean value indicating if the
/// end of buffer has been reached.
fn find_line_bounds(buffer: &Buffer, pos: usize) -> (usize, usize, bool) {
    let line_pos = nav::find_start_line(buffer, pos);
    let (next_pos, is_bottom) = nav::find_next_line(buffer, pos);
    (line_pos, next_pos, is_bottom)
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

/// Returns `line` formatted for display in the margin.
///
/// If `line` is small enough to fit within the margin, the formatted value will be
/// left-justified with leading spaces.
///
/// If `line` is too large to fit within the margin, the formatted value will only
/// contain the lower-order digits.
fn format_line(line: u32) -> String {
    if line < LINE_LIMIT {
        format!("{:>cols$} ", line, cols = MARGIN_COLS as usize - 1)
    } else {
        format!(
            "{:0>cols$}",
            line % 10_u32.pow(CLIP_LOWER_COLS),
            cols = CLIP_LOWER_COLS as usize,
        )
    }
}
