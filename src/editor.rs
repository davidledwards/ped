//! Provides a core set of editing functions over a buffer and an attachable window.
//!
//! An editor coordinates changes to and movement within a buffer and renders those
//! effects on the display of an attached window.

use crate::buffer::{Buffer, BufferRef};
use crate::config::ConfigurationRef;
use crate::nav::{self, Location};
use crate::render::{Align, Justify, Renderer, Rendering};
use crate::search::Pattern;
use crate::size::{Point, Size};
use crate::source::Source;
use crate::style::StyleBuilder;
use crate::syntax::Syntax;
use crate::token::{Cursor, Tokenizer, TokenizerRef};
use crate::window::{Window, WindowRef};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp;
use std::ops::Range;
use std::rc::Rc;
use std::time::Instant;

/// An editing session with an underlying [`Buffer`] and an attachable [`Window`].
pub struct Editor {
    /// Global configuration.
    config: ConfigurationRef,

    /// The source of the buffer.
    source: Source,

    /// Buffer containing the contents of this editor.
    buffer: BufferRef,

    /// An instance of the rendering engine.
    rendering: Box<dyn Renderer>,

    /// A style builder used by the rendering engine.
    styler: StyleBuilder,

    /// A value of `true` implies that _mutable_ operations are not allowed, though
    /// the notion of mutability is context-dependent and must be enforced by the
    /// caller.
    readonly: bool,

    /// A logical clock that increments with each change to the buffer.
    clock: u64,

    /// A stack containing changes to the buffer that can be _undone_, where each
    /// entry is associated with the value of `clock` prior to the change.
    undo: Vec<(u64, Change)>,

    /// A stack containing changes to the buffer that can be _redone_, where each
    /// entry is associated with the value of `clock` prior to the change.
    redo: Vec<(u64, Change)>,

    /// Tokenizes the buffer for syntax coloring.
    tokenizer: TokenizerRef,

    /// The number of milliseconds spent performing the last tokenization.
    tokenize_cost: u128,

    /// The value of [`clock`](Self::clock) at the time of the last tokenization.
    tokenize_clock: u64,

    /// A tokenization cursor that is always pointing to the top-left position on the
    /// display.
    syntax_cursor: Cursor,

    /// The value of `clock` when the buffer was most recently committed to storage.
    commit_clock: u64,

    /// An optional mark used when selecting text.
    mark: Option<Mark>,

    /// The window attached to this editor.
    window: WindowRef,

    /// Indicates whether _hard_ or _soft_ tabs are inserted.
    tab_hard: bool,

    /// The width of tab stops in number of columns.
    tab_cols: u32,

    /// Indicates that EOL characters should be written as `\r\n` if `true`, otherwise
    /// EOL is written as `\n`.
    crlf: bool,

    /// An optional last match from a prior search, which captures the starting buffer
    /// position of the match and the pattern used by the search.
    last_match: Option<(usize, Box<dyn Pattern>)>,

    /// A range in the buffer representing the current match from an active search
    /// operation, otherwise the value should be `None`.
    matched: Option<Range<usize>>,
}

pub type EditorRef = Rc<RefCell<Editor>>;

/// Used for building [`Editor`]s.
pub struct EditorBuilder {
    /// Required to build any editor.
    config: ConfigurationRef,

    /// Defaults to [`Source::Null`] if not explicitly set.
    source: Source,

    /// Defaults to `None`, which implies that an empty buffer will be created.
    buffer: Option<Buffer>,

    /// Defaults to `false`.
    readonly: bool,

    /// Defaults to the setting in `config`.
    rendering: Rendering,
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
    /// - indicates _soft_ selection
    RemoveSelectionBefore(usize, Vec<char>, bool),

    /// Represents the removal of selected text that comes after the cursor, where
    /// values are defined as:
    /// - buffer position prior to removal
    /// - text removed
    /// - indicates _soft_ selection
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

impl EditorBuilder {
    pub fn new(config: ConfigurationRef) -> EditorBuilder {
        let rendering = if config.settings.wrap {
            Rendering::Wrapping
        } else {
            Rendering::Scrolling
        };
        EditorBuilder {
            config,
            source: Source::Null,
            buffer: None,
            readonly: false,
            rendering,
        }
    }

    pub fn source(mut self, source: Source) -> EditorBuilder {
        self.source = source;
        self
    }

    pub fn buffer(mut self, buffer: Buffer) -> EditorBuilder {
        self.buffer = Some(buffer);
        self
    }

    pub fn readonly(mut self) -> EditorBuilder {
        self.readonly = true;
        self
    }

    pub fn build(self) -> Editor {
        Editor::new(
            self.config,
            self.source,
            self.buffer,
            self.readonly,
            self.rendering,
        )
    }
}

impl Capture {
    /// Returns a new capture with the mark cleared.
    pub fn without_mark(&self) -> Capture {
        Capture {
            mark: None,
            ..*self
        }
    }
}

impl Editor {
    /// An upper bound on the tolerable number of milliseconds to tokenize the
    /// buffer in real-time, otherwise the operation is deferred.
    const TOKENIZE_COST_LIMIT: u128 = 50;

    /// Creates a new, possibly `readonly` editor using `source`, an optional `buffer`
    /// which if `None` automatically creates an empty buffer, and the specified
    /// `rendering` engine.
    fn new(
        config: ConfigurationRef,
        source: Source,
        buffer: Option<Buffer>,
        readonly: bool,
        rendering: Rendering,
    ) -> Editor {
        // Create emoty buffer if necessary.
        let buffer = buffer.unwrap_or_default().into_ref();

        // Create renderer with unattached window.
        let rendering = rendering.create(config.clone(), buffer.clone());

        // Create styler for rendering text.
        let styler = StyleBuilder::new(config.clone());

        // Constructs syntax configuration based on type of buffer and file extension,
        // if applicable.
        let syntax = if let Source::File(path, _) = &source {
            config
                .registry
                .find(path)
                .cloned()
                .unwrap_or_else(Syntax::default)
        } else if let Source::Ephemeral(_) = &source {
            config
                .registry
                .find(source.to_string())
                .cloned()
                .unwrap_or_else(Syntax::default)
        } else {
            Syntax::default()
        };

        // Tokenize buffer.
        let mut tokenizer = Tokenizer::new(syntax);
        let timer = Instant::now();
        let syntax_cursor = tokenizer.tokenize(&buffer.borrow());
        let tokenize_cost = timer.elapsed().as_millis();

        // Additional settings.
        let tab_hard = config.settings.tab_hard;
        let tab_cols = config.settings.tab_size;
        let crlf = config.settings.crlf;

        Editor {
            config,
            source,
            buffer,
            rendering,
            styler,
            readonly,
            clock: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            tokenizer: tokenizer.into_ref(),
            tokenize_cost,
            tokenize_clock: 0,
            syntax_cursor,
            commit_clock: 0,
            mark: None,
            window: Window::zombie().into_ref(),
            tab_hard,
            tab_cols,
            crlf,
            last_match: None,
            matched: None,
        }
    }

    /// Turns the editor into a [`EditorRef`].
    pub fn into_ref(self) -> EditorRef {
        Rc::new(RefCell::new(self))
    }

    /// Returns `true` if editor can be modified.
    pub fn is_mutable(&self) -> bool {
        !self.readonly
    }

    /// Clears the contents of the buffer and resets internal state as if the editor was
    /// just created.
    pub fn reset(&mut self) {
        *self = Self::new(
            self.config.clone(),
            self.source.clone(),
            None,
            self.readonly,
            self.rendering.kind(),
        );
    }

    /// Returns a duplicate of this editor using `source`.
    ///
    /// Specifically, the buffer is cloned as well as the current buffer position and
    /// cursor values. All other attributes are initialized as if a new editor were
    /// being created.
    pub fn duplicate(&self, source: Source) -> Editor {
        let mut buffer = self.buffer().clone();
        buffer.set_pos(self.pos());
        Self::new(
            self.config.clone(),
            source,
            Some(buffer),
            false,
            self.rendering.kind(),
        )
    }

    /// Returns a reference to the source.
    #[inline]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Changes the `source` associated with this editor.
    pub fn assume(&mut self, source: Source) {
        self.source = source;
    }

    /// Returns a reference to the underlying buffer.
    #[inline]
    pub fn buffer(&self) -> Ref<'_, Buffer> {
        self.buffer.borrow()
    }

    /// Returns `true` if the buffer has changed.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        // It might seem reasonable to expect `clock > commit_clock` is more correct
        // when determining dirtiness of buffer since it would not make sense that
        // commit_clock occurs after most recent change. However, changes in undo
        // stack can be applied after buffer is committed to storage, making it possible
        // for commit_clock to appear after latest change.
        self.clock != self.commit_clock
    }

    /// Makes this editor _not_ dirty.
    pub fn clear_dirty(&mut self) {
        self.commit_clock = self.clock;
        self.show_banner();
    }

    /// Returns the cursor position on the display in terms of _row_ and _column_.
    ///
    /// The _row_ and _column_ values are `0`-based and exclusively bounded by
    /// [`size()`](Self::size).
    #[inline]
    pub fn cursor(&self) -> Point {
        self.rendering.cursor()
    }

    /// Returns the location of the cursor position in the buffer.
    #[inline]
    pub fn location(&self) -> Location {
        self.rendering.location()
    }

    /// Returns the number of rows available on the editor canvas.
    #[inline]
    pub fn rows(&self) -> u32 {
        self.rendering.rows()
    }

    /// Returns the size of the editor canvas.
    #[inline]
    pub fn size(&self) -> Size {
        self.rendering.size()
    }

    /// Returns the buffer position corresponding to the [`cursor`](Self::cursor).
    #[inline]
    pub fn pos(&self) -> usize {
        self.rendering.pos()
    }

    /// Returns `true` if the tab mode is _hard_ and `false` if _soft_.
    pub fn get_tab(&self) -> bool {
        self.tab_hard
    }

    /// Sets the tab mode based on the value of `hard`.
    pub fn set_tab(&mut self, hard: bool) {
        self.tab_hard = hard;
        self.window
            .borrow()
            .banner
            .borrow_mut()
            .set_tab(hard)
            .draw();
    }

    /// Returns `true` if EOL characters should be written as `\r\n`.
    pub fn get_crlf(&self) -> bool {
        self.crlf
    }

    /// Sets the CRLF behavior based on the value of `crlf`.
    pub fn set_crlf(&mut self, crlf: bool) {
        self.crlf = crlf;
        self.window
            .borrow()
            .banner
            .borrow_mut()
            .set_eol(crlf)
            .draw();
    }

    /// Attaches `window` to this editor and positions the cursor as instructed by
    /// `align`.
    pub fn attach(&mut self, window: WindowRef, align: Align) {
        self.window = window.clone();
        self.rendering.attach(window);

        // Align cursor and draw contents only if window is not a zombie.
        if self.rendering.is_attached() {
            self.rendering.align_cursor(align, Justify::Auto);
            self.draw();
        }
    }

    /// Detaches the existing window from this editor.
    pub fn detach(&mut self) {
        self.rendering.detach();
    }

    /// Sets the cursor location and corresponding buffer position to `cursor`, though
    /// the final cursor location is constrained by end-of-line and end-of-buffer
    /// boundaries.
    ///
    /// This function was designed for responding to _mouse click_ events where the
    /// position of the click is captured in `cursor`.
    ///
    /// The coordinates in `cursor` are presumed to be relative to the origin of the
    /// editor canvas.
    pub fn focus_cursor(&mut self, cursor: Point) {
        self.rendering.focus_cursor(cursor);
    }

    /// Sets the position of the cursor based on the `align` and `justify` objectives.
    pub fn align_cursor(&mut self, align: Align, justify: Justify) {
        self.rendering.align_cursor(align, justify);
        self.align_syntax();
    }

    /// Draws the canvas and banner regardless of whether any updates have occurred.
    pub fn draw(&mut self) {
        self.window.borrow().canvas.borrow_mut().clear();
        self.show_banner();
        self.render();
    }

    /// Makes the cursor visible.
    pub fn show_cursor(&mut self) {
        self.rendering.show_cursor();
    }

    /// Tries to move the cursor _backward_ from the current buffer position by `len`
    /// characters.
    pub fn move_backward(&mut self, len: usize) {
        let pos = self.pos().saturating_sub(len);
        if pos < self.pos() {
            self.move_to(pos, Align::Auto, Justify::Auto);
        }
    }

    /// Tries to move the cursor _forward_ from the current buffer position by `len`
    /// characters.
    pub fn move_forward(&mut self, len: usize) {
        let cur_pos = self.pos();
        let pos = cmp::min(cur_pos + len, self.buffer().size());
        if pos > cur_pos {
            self.move_to(pos, Align::Auto, Justify::Auto);
        }
    }

    /// Tries to move the cursor _backward_ by one word from the current buffer
    /// position.
    pub fn move_backward_word(&mut self) {
        let pos = self.find_word_before(self.pos());
        if pos < self.pos() {
            self.move_to(pos, Align::Auto, Justify::Auto);
        }
    }

    /// Tries to move the cursor _forward_ by one word from the current buffer
    /// position.
    pub fn move_forward_word(&mut self) {
        let pos = self.find_word_after(self.pos());
        if pos > self.pos() {
            self.move_to(pos, Align::Auto, Justify::Auto);
        }
    }

    /// Tries to move the cursor _up_ by the specified number of `try_rows`.
    ///
    /// If `pin` is `true`, then the cursor will remain on the current row if the
    /// resulting display makes it possible. Pinning is useful when _paging up_.
    ///
    /// If `pin` is `false`, then the cursor will move up in tandem with `try_rows`,
    /// though not to extend beyond the top of the display.
    pub fn move_up(&mut self, try_rows: u32, pin: bool) {
        if self.rendering.move_up(try_rows, pin) > 0 {
            self.align_syntax();
        }
    }

    /// Tries to move the cursor _down_ by the specified number of `try_rows`.
    ///
    /// If `pin` is `true`, then the cursor will remain on the current row. Pinning is
    /// useful when _paging down_.
    ///
    /// If `pin` is `false`, then the cursor will move down in tandem with `try_rows`,
    /// though not to extend beyond the bottom of the display.
    pub fn move_down(&mut self, try_rows: u32, pin: bool) {
        if self.rendering.move_down(try_rows, pin) > 0 {
            self.align_syntax();
        }
    }

    /// Moves the cursor to the _start_ of the current row.
    pub fn move_start(&mut self) {
        self.rendering.move_start();
    }

    /// Moves the cursor to the _end_ of the current row.
    pub fn move_end(&mut self) {
        self.rendering.move_end();
    }

    /// Moves the cursor to the _top_ of the buffer.
    pub fn move_top(&mut self) {
        self.move_to(0, Align::Top, Justify::Auto);
    }

    /// Moves the cursor to the _bottom_ of the buffer.
    pub fn move_bottom(&mut self) {
        let pos = self.buffer().size();
        self.move_to(pos, Align::Bottom, Justify::Auto);
    }

    /// Moves the buffer position to the location `loc`, and places the cursor on
    /// the display according to the `align` and `justify` objectives.
    pub fn move_location(&mut self, loc: Location, align: Align, justify: Justify) {
        let pos = nav::find_pos(&self.buffer(), loc);
        self.move_to(pos, align, justify);
    }

    /// Moves the current buffer position to `pos` and places the cursor on the
    /// display according to the `align` and `justify` objectives.
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
    pub fn move_to(&mut self, pos: usize, align: Align, justify: Justify) {
        self.rendering.move_to(pos, align, justify);
        self.align_syntax();
    }

    /// Tries scrolling the contents of the display in an _upward_ direction by the
    /// specified number of `try_rows` while also trying to preserve the cursor position,
    /// returning the actual number of rows scrolled.
    ///
    /// Conceptually, this function moves the viewable area towards the top of the
    /// buffer.
    pub fn scroll_up(&mut self, try_rows: u32) -> u32 {
        let rows = self.rendering.scroll_up(try_rows);
        if rows > 0 {
            self.align_syntax();
        }
        rows
    }

    /// Tries scrolling the contents of the display in a _downward_ direction by the
    /// specified number of `try_rows` while also trying to preserve the cursor position,
    /// returning the actual number of rows scrolled.
    ///
    /// Conceptually, this function moves the viewable area towards the bottom of the
    /// buffer.
    pub fn scroll_down(&mut self, try_rows: u32) -> u32 {
        let rows = self.rendering.scroll_down(try_rows);
        if rows > 0 {
            self.align_syntax();
        }
        rows
    }

    /// Tries scrolling the contents of the display in a _leftward_ direction by the
    /// specified number of `try_cols` while also trying to preserve the cursor position,
    /// returning the actual number of columns scrolled.
    ///
    /// Conceptually, this function moves the viewable area towards the rightmost column
    /// of the current row.
    pub fn scroll_left(&mut self, try_cols: u32) -> u32 {
        let cols = self.rendering.scroll_left(try_cols);
        if cols > 0 {
            self.align_syntax();
        }
        cols
    }

    /// Tries scrolling the contents of the display in a _rightward_ direction by the
    /// specified number of `try_cols` while also trying to preserve the cursor position,
    /// returning the actual number of columns scrolled.
    ///
    /// Conceptually, this function moves the viewable area towards the leftmost column
    /// of the current row.
    pub fn scroll_right(&mut self, try_cols: u32) -> u32 {
        let cols = self.rendering.scroll_right(try_cols);
        if cols > 0 {
            self.align_syntax();
        }
        cols
    }

    /// Sets a _hard_ mark at the current buffer position and returns the previous
    /// mark if set.
    pub fn set_hard_mark(&mut self) -> Option<Mark> {
        self.mark.replace(Mark(self.pos(), false))
    }

    /// Sets a _soft_ mark at the current buffer position unless a _soft_ mark was
    /// previously set.
    ///
    /// Note that if a _hard_ mark was previously set, the _soft_ mark will replace
    /// it.
    ///
    /// Returns the previous _hard_ mark if set, otherwise `None`.
    pub fn set_soft_mark(&mut self) -> Option<Mark> {
        if let Some(mark @ Mark(_, soft)) = self.mark {
            if soft {
                None
            } else {
                self.mark = Some(Mark(self.pos(), true));
                Some(mark)
            }
        } else {
            self.mark = Some(Mark(self.pos(), true));
            None
        }
    }

    /// Sets a _soft_ mark at buffer position `pos` unless a _soft_ mark was previously
    /// set.
    ///
    /// Note that if a _hard_ mark was previously set, the _soft_ mark will replace
    /// it.
    ///
    /// Returns the previous _hard_ mark if set, otherwise `None`.
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

    /// Clears and returns the mark if _soft_, otherwise `None` is returned.
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

    /// Returns the text between the current buffer position and `mark`.
    pub fn copy_mark(&self, mark: Mark) -> Vec<char> {
        let Range { start, end } = self.get_mark_range(mark);
        self.copy(start, end)
    }

    /// Returns the buffer range of `mark` relative to the current buffer position.
    fn get_mark_range(&self, mark: Mark) -> Range<usize> {
        let Mark(pos, _) = mark;
        if pos < self.pos() {
            pos..self.pos()
        } else {
            self.pos()..pos
        }
    }

    /// Returns the text of the line on which the current buffer position rests.
    pub fn copy_line(&self) -> Vec<char> {
        let (start_pos, end_pos) = self.rendering.line();
        self.copy(start_pos, end_pos)
    }

    /// Returns the text between `from_pos` and `end_pos`.
    ///
    /// Specifically, the range of characters is bounded _inclusively below_ and
    /// _exclusively above_. If `from_pos` is less than `to_pos`, then the range is
    /// [`from_pos`, `to_pos`), otherwise it is [`to_pos`, `from_pos`).
    ///
    /// This function will return an empty vector if `from_pos` is equal to `to_pos`.
    pub fn copy(&self, from_pos: usize, to_pos: usize) -> Vec<char> {
        self.buffer().copy(from_pos, to_pos)
    }

    /// Reverts the last change to the buffer, if any, and makes that change eligible
    /// to be reapplied via [`redo`](Editor::redo).
    ///
    /// Returns `true` if the change was reverted and `false` if the _undo_ stack is
    /// empty.
    pub fn undo(&mut self) -> bool {
        if let Some((clock, change)) = self.undo.pop() {
            self.undo_change(&change);
            self.redo.push((clock, change));

            // Restored clock must represent pre-change value since undo is as if
            // change was never applied by user.
            self.clock = clock;
            true
        } else {
            false
        }
    }

    /// Applies the last change to the buffer, if any, that was reverted via
    /// [`undo`](Editor::undo).
    ///
    /// Returns `true` if the change was applies and `false` if the _redo_ stack is
    /// empty.
    pub fn redo(&mut self) -> bool {
        if let Some((clock, change)) = self.redo.pop() {
            self.redo_change(&change);
            self.undo.push((clock, change));

            // Restored clock must represent post-change value since redo is
            // effectively equivalent to change entered by user.
            self.clock = clock + 1;
            true
        } else {
            false
        }
    }

    /// Returns the captured state of the editor.
    pub fn capture(&self) -> Capture {
        Capture {
            pos: self.pos(),
            cursor: self.cursor(),
            mark: self.mark,
        }
    }

    /// Restores the editor to the captured state in `capture`.
    ///
    /// Note that if the editor changes after state has been captured, there is no
    /// guarantee that said state will be restored precisely as it was.
    pub fn restore(&mut self, capture: &Capture) {
        self.move_to(
            capture.pos,
            Align::Row(capture.cursor.row),
            Justify::Col(capture.cursor.col),
        );
        if let Some(Mark(pos, soft)) = capture.mark {
            let pos = cmp::min(pos, self.buffer().size());
            self.mark = Some(Mark(pos, soft));
        } else {
            self.mark = None;
        }
    }

    /// Tokenizes the buffer if changes occurred since the last tokenization, returning
    /// `true` if tokenization occurred and `false` otherwise.
    pub fn tokenize(&mut self) -> bool {
        if self.tokenize_clock != self.clock {
            self.possibly_tokenize(true);
            true
        } else {
            false
        }
    }

    /// Renders the contents of the editor.
    pub fn render(&mut self) {
        // Construct range for possibly selected text.
        let selected = self.mark.map(|Mark(mark_pos, _)| {
            if mark_pos < self.pos() {
                mark_pos..self.pos()
            } else {
                self.pos()..mark_pos
            }
        });

        // Create styler for rendering engine that captures state of selected and
        // matched regions of text.
        let style = self.styler.style(selected, self.matched.clone());

        // Render text.
        self.rendering
            .render(&self.tokenizer.borrow(), self.syntax_cursor, &style);

        // Renders additional information.
        self.window
            .borrow()
            .banner
            .borrow_mut()
            .set_dirty(self.is_dirty())
            .set_char(self.buffer().get_char(self.pos()))
            .set_location(self.location())
            .draw();
    }

    /// Sets the last match from a prior search, where `pos` is the starting positon of
    /// the match and `pattern` is the applicable search pattern.
    pub fn set_last_match(&mut self, pos: usize, pattern: Box<dyn Pattern>) {
        self.last_match = Some((pos, pattern));
    }

    /// Takes the last match from a prior search.
    pub fn take_last_match(&mut self) -> Option<(usize, Box<dyn Pattern>)> {
        self.last_match.take()
    }

    /// Sets the matched range in the buffer from `start_pos` to `end_pos`.
    pub fn set_matched(&mut self, start_pos: usize, end_pos: usize) {
        self.matched = Some(start_pos..end_pos);
    }

    /// Clears the matched range.
    pub fn clear_matched(&mut self) {
        self.matched = None;
    }

    /// Inserts the character `c` at the current buffer position.
    pub fn insert_char(&mut self, c: char) {
        self.insert_normal(&[c])
    }

    /// Inserts the string slice `str` at the current buffer position.
    pub fn insert_str(&mut self, text: &str) {
        self.insert_normal(&text.chars().collect::<Vec<_>>())
    }

    /// Inserts the `TAB` character.
    pub fn insert_tab(&mut self) {
        if self.tab_hard {
            self.insert_char('\t');
        } else {
            let n = self.tab_cols - (self.location().col % self.tab_cols);
            self.insert_str(&" ".repeat(n as usize));
        }
    }

    /// Inserts the array of `text` at the current buffer position.
    pub fn insert(&mut self, text: &[char]) {
        self.insert_normal(text);
    }

    /// Removes and returns the character before the current buffer position.
    ///
    /// An empty vector is returned if the current position is already at the top
    /// of the buffer.
    pub fn remove_before(&mut self) -> Vec<char> {
        if self.pos() > 0 {
            self.remove(self.pos() - 1)
        } else {
            vec![]
        }
    }

    /// Removes and returns the character after the current buffer position.
    ///
    /// An empty vector is returned if the current position is already at the
    /// bottom of the buffer.
    pub fn remove_after(&mut self) -> Vec<char> {
        if self.pos() < self.buffer().size() {
            self.remove(self.pos() + 1)
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
        let (start_pos, end_pos) = self.rendering.line();
        self.move_to(start_pos, Align::Auto, Justify::Auto);
        self.remove(end_pos)
    }

    /// Removes and returns the text between the start of the current line and the
    /// current buffer position.
    pub fn remove_start(&mut self) -> Vec<char> {
        let (start_pos, _) = self.rendering.line();
        if self.pos() == start_pos {
            self.remove_before()
        } else {
            self.remove(start_pos)
        }
    }

    /// Removes and returns the text between the current buffer position and the end
    /// of the current line.
    pub fn remove_end(&mut self) -> Vec<char> {
        let (_, end_pos) = self.rendering.line();
        self.remove(end_pos)
    }

    /// Removes and returns the text between the current buffer position and `pos`.
    ///
    /// Specifically, the range of characters is bounded _inclusively below_ and
    /// _exclusively above_. If `pos` is less than the current buffer position, then
    /// the range is [`pos`, `cur_pos`), otherwise it is [`cur_pos`, `pos`).
    ///
    /// This function will return an empty vector if `pos` is equal to `cur_pos`.
    pub fn remove(&mut self, pos: usize) -> Vec<char> {
        self.remove_internal(pos, Some(Log::Normal))
    }

    /// Aligns the syntax cursor with the top line.
    fn align_syntax(&mut self) {
        self.syntax_cursor = self
            .tokenizer
            .borrow()
            .find(self.syntax_cursor, self.rendering.origin());
    }

    /// Sets the values of all banner attributes and draws it.
    fn show_banner(&mut self) {
        self.window
            .borrow()
            .banner
            .borrow_mut()
            .set_dirty(self.is_dirty())
            .set_source(self.source.clone())
            .set_syntax(self.tokenizer().syntax().name.clone())
            .set_eol(self.crlf)
            .set_tab(self.tab_hard)
            .set_char(self.buffer().get_char(self.pos()))
            .set_location(self.location())
            .draw();
    }

    /// Returns the position of the word that comes before `pos`.
    fn find_word_before(&self, pos: usize) -> usize {
        self.buffer()
            .backward(pos)
            .index()
            .skip_while(|(_, c)| c.is_whitespace())
            .find(|(_, c)| c.is_whitespace())
            .map(|(pos, _)| pos + 1)
            .unwrap_or(0)
    }

    /// Returns the position of the word that follows after `pos`.
    fn find_word_after(&self, pos: usize) -> usize {
        self.buffer()
            .forward(pos)
            .index()
            .skip_while(|(_, c)| !c.is_whitespace())
            .find(|(_, c)| !c.is_whitespace())
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
            self.buffer_mut().set_pos(self.pos());
            if text.len() == 1 {
                self.buffer_mut().insert_char(text[0]);
            } else {
                self.buffer_mut().insert(text);
            }

            // Log change to buffer.
            if log.is_some() {
                self.log(Change::Insert(self.pos(), text.to_vec()));
                self.clock = cmp::max(self.clock, self.commit_clock) + 1;
            }

            // Update tokenizer with insertion range.
            self.syntax_cursor = {
                let mut tokenizer = self.tokenizer_mut();
                let cursor = tokenizer.find(self.syntax_cursor, self.pos());
                tokenizer.insert(cursor, text.len())
            };

            // Inform renderer that text has been inserted,
            self.rendering.insert(text.len());
            self.possibly_tokenize(false);
        }
    }

    /// An internal workhorse to which all _removal_ functions delegate.
    ///
    /// A `log` value of `None` indicates that the change is not recorded in the undo
    /// stack.
    fn remove_internal(&mut self, pos: usize, log: Option<Log>) -> Vec<char> {
        if pos == self.pos() {
            vec![]
        } else {
            // Form range depending on location of `pos` relative to current buffer
            // position.
            let cur_pos = self.pos();
            let pos = cmp::min(pos, self.buffer().size());
            let (from_pos, len) = if pos < cur_pos {
                // Prior to removing text, move cursor and buffer position to `pos`
                // since it appears before current buffer position. This happens to be
                // precondition for calling rendering remove function, which assumes
                // range of removed text starts at current buffer position.
                self.rendering.move_to(pos, Align::Auto, Justify::Auto);
                (pos, cur_pos - pos)
            } else {
                (cur_pos, pos - cur_pos)
            };

            // Common use case of single-character removal allows more efficient buffer
            // function to be used.
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
                        self.log(if pos < cur_pos {
                            Change::RemoveBefore(cur_pos, text.clone())
                        } else {
                            Change::RemoveAfter(cur_pos, text.clone())
                        });
                    }
                    Log::Selection(soft) => {
                        self.log(if pos < cur_pos {
                            Change::RemoveSelectionBefore(cur_pos, text.clone(), soft)
                        } else {
                            Change::RemoveSelectionAfter(cur_pos, text.clone(), soft)
                        });
                    }
                }
                self.clock = cmp::max(self.clock, self.commit_clock) + 1;
            }

            // Update tokenizer with removal range.
            self.syntax_cursor = {
                let mut tokenizer = self.tokenizer_mut();
                let cursor = tokenizer.find(self.syntax_cursor, from_pos);
                tokenizer.remove(cursor, text.len())
            };

            // Inform renderer that text has been been removed.
            self.rendering.remove();
            self.possibly_tokenize(false);
            text
        }
    }

    /// Tokenizes the buffer if either the prior tokenization fell below the real-time
    /// limit or `force` is `true`, otherwise the operation is not performed.
    fn possibly_tokenize(&mut self, force: bool) {
        if force || self.tokenize_cost < Self::TOKENIZE_COST_LIMIT {
            self.syntax_cursor = {
                let timer = Instant::now();
                let cursor = self.tokenizer_mut().tokenize(&self.buffer());
                self.tokenize_cost = timer.elapsed().as_millis();
                cursor
            };
            self.tokenize_clock = self.clock;
        }
        self.align_syntax();
    }

    /// Reverts `change`.
    fn undo_change(&mut self, change: &Change) {
        match change {
            Change::Insert(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto, Justify::Auto);
                self.remove_internal(pos + text.len(), None);
            }
            Change::RemoveBefore(pos, text) => {
                self.clear_mark();
                self.move_to(pos - text.len(), Align::Auto, Justify::Auto);
                self.insert_internal(text, None);
            }
            Change::RemoveAfter(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto, Justify::Auto);
                self.insert_internal(text, None);
                self.move_to(*pos, Align::Auto, Justify::Auto);
            }
            Change::RemoveSelectionBefore(pos, text, soft) => {
                self.move_to(pos - text.len(), Align::Auto, Justify::Auto);
                if *soft {
                    self.set_soft_mark();
                } else {
                    self.set_hard_mark();
                }
                self.insert_internal(text, None);
            }
            Change::RemoveSelectionAfter(pos, text, soft) => {
                self.move_to(*pos, Align::Auto, Justify::Auto);
                self.insert_internal(text, None);
                if *soft {
                    self.set_soft_mark();
                } else {
                    self.set_hard_mark();
                }
                self.move_to(*pos, Align::Auto, Justify::Auto);
            }
        }
    }

    /// Applies `change`.
    fn redo_change(&mut self, change: &Change) {
        match change {
            Change::Insert(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto, Justify::Auto);
                self.insert_internal(text, None);
            }
            Change::RemoveBefore(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto, Justify::Auto);
                self.remove_internal(pos - text.len(), None);
            }
            Change::RemoveAfter(pos, text) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto, Justify::Auto);
                self.remove_internal(pos + text.len(), None);
            }
            Change::RemoveSelectionBefore(pos, text, _) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto, Justify::Auto);
                self.remove_internal(pos - text.len(), None);
            }
            Change::RemoveSelectionAfter(pos, text, _) => {
                self.clear_mark();
                self.move_to(*pos, Align::Auto, Justify::Auto);
                self.remove_internal(pos + text.len(), None);
            }
        }
    }

    /// Logs `change` by pushing it onto the _undo_ stack and clearing the _redo_
    /// stack.
    fn log(&mut self, change: Change) {
        const UNDO_SOFT_LIMIT: usize = 1024;
        const UNDO_HARD_LIMIT: usize = 1280;

        if let Some((clock, top)) = self.undo.pop() {
            if let Some(combined) = change.possibly_combine(&top) {
                // Use clock value of prior change when current change is combined
                // since subsequent undo would restore all combined changes.
                self.undo.push((clock, combined));
            } else {
                // Restore original change since latest change cannot be combined.
                self.undo.push((clock, top));
                self.undo.push((self.clock, change));
            }
        } else {
            // Capture clock value which represents state prior to change.
            self.undo.push((self.clock, change));
        }
        self.redo.clear();

        // Trim undo stack to soft limit once size exceeds hard limit, as this avoids
        // repeatedly trimming with every change.
        if self.undo.len() > UNDO_HARD_LIMIT {
            let n = self.undo.len() - UNDO_SOFT_LIMIT;
            self.undo.drain(0..n);
        }
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
}
