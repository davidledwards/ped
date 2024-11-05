//! Editor.
use crate::buffer::{Buffer, BufferRef};
use crate::canvas::{Canvas, CanvasRef};
use crate::grid::Cell;
use crate::size::{Point, Size};
use crate::theme::ThemeRef;
use crate::window::{BannerRef, Window, WindowRef};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp;
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

    /// Attached window.
    view: View,
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
    /// which must always be >= `row_pos`.
    row_pos: usize,

    /// Length of the display line, excluding the `\n` if one exists.
    row_len: usize,

    /// Buffer position corresponding to the first character of the buffer line,
    /// which must always be <= `row_pos`.
    line_pos: usize,

    /// Length of the buffer line, excluding the `\n` if one exists.
    line_len: usize,

    /// The `0`-based number of the buffer line.
    line_nbr: usize,
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

struct View {
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

    /// Cached value of *blank* cell honoring the color [`View::theme`].
    blank_cell: Cell,
}

impl Line {
    fn zero() -> Line {
        Line {
            row_pos: 0,
            row_len: 0,
            line_pos: 0,
            line_len: 0,
            line_nbr: 0,
        }
    }

    /// Returns `true` if the row of this line points to the top of the buffer.
    #[inline]
    fn is_top(&self) -> bool {
        self.row_pos == 0
    }

    /// Returns `true` if the row of this line has wrapped from at least one prior
    /// row.
    #[inline]
    fn has_wrapped(&self) -> bool {
        self.row_pos > self.line_pos
    }

    /// Returns a possibly smaller value of `col` if it extends beyond the end of
    /// the row.
    #[inline]
    fn snap_col(&self, col: u32) -> u32 {
        cmp::min(col, self.row_len as u32)
    }

    /// Returns the buffer position of `col` relative to the starting position of
    /// the row.
    #[inline]
    fn pos_of(&self, col: u32) -> usize {
        self.row_pos + col as usize
    }

    /// Returns the column number of `pos` relative to the starting position of the
    /// row, but be advised that the column may extend beyond the end of the row.
    #[inline]
    fn col_of(&self, pos: usize) -> u32 {
        (pos - self.row_pos) as u32
    }

    #[inline]
    fn end_pos(&self) -> usize {
        self.row_pos + self.row_len
    }

    fn point_of(&self, col: u32) -> Point {
        Point::new(
            self.line_nbr as u32 + 1,
            (self.row_pos - self.line_pos) as u32 + col + 1,
        )
    }
}

impl View {
    fn new(window: WindowRef) -> View {
        let theme = window.borrow().theme().clone();
        let canvas = window.borrow().canvas().clone();
        let banner = window.borrow().banner().clone();
        let Size { rows, cols } = canvas.borrow().size();
        let blank_cell = Cell::new(' ', theme.text_color);

        View {
            theme,
            canvas,
            banner,
            rows,
            cols,
            blank_cell,
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
            top_line: Line::zero(),
            cur_line: Line::zero(),
            snap_col: None,
            cursor: Point::ORIGIN,
            view: View::new(Window::zombie().to_ref()),
        }
    }

    /// Turns the editor into a [`EditorRef`].
    pub fn to_ref(self) -> EditorRef {
        Rc::new(RefCell::new(self))
    }

    /// Attaches `window` to this editor.
    pub fn attach(&mut self, window: WindowRef) {
        let is_zombie = window.borrow().is_zombie();
        self.view = View::new(window);

        if !is_zombie {
            let title = match self.path {
                Some(ref path) => path.to_string_lossy().to_string(),
                None => "new".to_string(),
            };

            self.align_cursor(Align::Auto);

            self.view
                .banner
                .borrow_mut()
                .set_title(title)
                .set_cursor(self.cur_line.point_of(self.cursor.col))
                .draw();

            self.draw();
        }
    }

    fn set_top_line(&mut self, try_row: u32) -> u32 {
        self.top_line = self.cur_line.clone();
        for row in 0..try_row {
            if let Some(line) = self.prev_line(&self.top_line) {
                self.top_line = line;
            } else {
                return row;
            }
        }
        try_row
    }

    pub fn align_cursor(&mut self, align: Align) {
        // Determine ideal row where cursor would like to be focused, though this should
        // be considered a hint.
        let try_row = match align {
            Align::Auto => cmp::min(self.cursor.row, self.view.rows - 1),
            Align::Center => self.view.rows / 2,
            Align::Top => 0,
            Align::Bottom => self.view.rows - 1,
        };

        // Tries to position cursor on target row, but no guarantee depending on proximity
        // of row to top of buffer.
        self.cur_line = self.find_line(self.cur_pos);
        let row = self.set_top_line(try_row);
        let col = self.cur_line.col_of(self.cur_pos);
        self.snap_col = None;
        self.cursor = Point::new(row, col);
    }

    pub fn draw(&mut self) {
        self.canvas_mut().clear();
        self.render();
    }

    pub fn get_size(&self) -> Size {
        (self.view.rows, self.view.cols).into()
    }

    pub fn get_cursor(&self) -> Point {
        self.cursor
    }

    pub fn show_cursor(&mut self) {
        self.canvas_mut().set_cursor(self.cursor);
    }

    pub fn insert_char(&mut self, c: char) {
        self.insert(&[c])
    }

    pub fn insert_str(&mut self, text: &str) {
        self.insert(&text.chars().collect::<Vec<_>>())
    }

    pub fn insert(&mut self, cs: &[char]) {
        if cs.len() > 0 {
            self.buffer_mut().set_pos(self.cur_pos);
            let cur_pos = if cs.len() == 1 {
                self.buffer_mut().insert_char(cs[0])
            } else {
                self.buffer_mut().insert(cs)
            };

            // update the current line since insertion will changed info
            self.cur_line = self.update_line(&self.cur_line);

            let rows = self.find_down_cur_line(cur_pos);

            let row = self.cursor.row + rows;
            let row = if row < self.view.rows {
                // this means the new cursor has not moved beyond the bottom
                // however, we need to update the top line in case it was affected by
                // the insertion
                self.top_line = self.update_line(&self.top_line);
                row
            } else {
                // new row is beyond bottom, so find the new top line
                self.set_top_line(self.view.rows - 1)
            };

            let col = self.cur_line.col_of(cur_pos);
            self.cur_pos = cur_pos;
            self.snap_col = None;
            self.cursor = Point::new(row, col);
            self.render();
        }
    }

    pub fn remove_left(&mut self) -> Vec<char> {
        if self.cur_pos > 0 {
            self.remove(self.cur_pos - 1)
        } else {
            vec![]
        }
    }

    pub fn remove_right(&mut self) -> Vec<char> {
        if self.cur_pos < self.buffer().size() {
            self.remove(self.cur_pos + 1)
        } else {
            vec![]
        }
    }

    /// Removes and returns characters from `from_pos` to the cursor position.
    ///
    /// Specifically, characters in the range [`from_pos`, `self.cur_pos`) are removed
    /// if `from_pos` is less than `self.cur_pos`, otherwise the operation is ignored
    /// and an empty vector is returned.
    ///
    /// Removes and returns characters from the cursor position to `to_pos`.
    ///
    /// Specifically, characters in the range [`self.cur_pos`, `to_pos`) are removed
    /// if `to_pos` is greater than `self.cur_pos`, otherwise the operation is ignored
    /// and an empty vector is returned.
    pub fn remove(&mut self, pos: usize) -> Vec<char> {
        if pos == self.cur_pos {
            vec![]
        } else {
            let pos = cmp::min(pos, self.buffer().size());
            let (from_pos, len) = if pos < self.cur_pos {
                (pos, self.cur_pos - pos)
            } else {
                (self.cur_pos, pos - self.cur_pos)
            };

            let row = if from_pos < self.cur_pos {
                // backtrack to find cur line that contains from_pos
                let rows = self.find_up_cur_line(from_pos);
                if rows > self.cursor.row {
                    // new row is above top
                    self.set_top_line(0)
                } else {
                    // new row is still visible
                    self.cursor.row - rows
                }
            } else {
                // cursor will remain on same row
                self.cursor.row
            };

            self.buffer_mut().set_pos(from_pos);
            let text = if len == 1 {
                vec![self.buffer_mut().remove_char().unwrap()]
            } else {
                self.buffer_mut().remove(len)
            };

            // both lines must be updated after removal since the information may have
            // changed
            self.cur_line = self.update_line(&self.cur_line);
            self.top_line = self.update_line(&self.top_line);

            let col = self.cur_line.col_of(from_pos);
            self.cur_pos = from_pos;
            self.snap_col = None;
            self.cursor = Point::new(row, col);
            self.render();
            text
        }
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

    pub fn move_up(&mut self, try_rows: u32, pin: bool) {
        let rows = self.up_cur_line(try_rows);
        if rows > 0 {
            let row = if pin {
                if rows < try_rows {
                    // this cursor reached the top of the buffer before the desired number
                    // of rows could be processed, so the resulting row is always 0
                    self.set_top_line(0)
                } else {
                    // try to find the new top pos by stepping backwards by the cursor.row
                    // number of rows
                    self.set_top_line(self.cursor.row)
                }
            } else {
                if rows > self.cursor.row {
                    // new row is above current top, so just make current row the top
                    self.set_top_line(0)
                } else {
                    // new row does not require a change in the top
                    self.cursor.row - rows
                }
            };

            let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
            let col = self.cur_line.snap_col(try_col);
            self.cur_pos = self.cur_line.pos_of(col);
            self.snap_col = Some(try_col);
            self.cursor = Point::new(row, col);
            self.render();
        }
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

    pub fn move_down(&mut self, try_rows: u32, pin: bool) {
        let rows = self.down_cur_line(try_rows);
        if rows > 0 {
            let row = if pin {
                // just move top line down by same number of rows
                self.down_top_line(rows);
                self.cursor.row
            } else {
                if self.cursor.row + rows < self.view.rows {
                    // this means the new cursor has not moved beyond the bottom, and the
                    // current top line does not change
                    self.cursor.row + rows
                } else {
                    // new row is beyond bottom, so we need to find the new top line
                    self.set_top_line(self.view.rows - 1)
                }
            };

            let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
            let col = self.cur_line.snap_col(try_col);
            self.cur_pos = self.cur_line.pos_of(col);
            self.snap_col = Some(try_col);
            self.cursor = Point::new(row, col);
            self.render();
        }
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

    fn find_down_cur_line(&mut self, pos: usize) -> u32 {
        let mut rows = 0;
        while pos > self.cur_line.end_pos()
            || pos == self.cur_line.row_pos + self.view.cols as usize
        {
            self.cur_line = self.next_line_unchecked(&self.cur_line);
            rows += 1;
        }
        rows
    }

    pub fn move_to(&mut self, pos: usize, align: Align) {
        let row = if pos < self.top_line.row_pos {
            // pos is above top of page, so find new top line and cur line
            self.find_up_top_line(pos);

            let rows = match align {
                Align::Top | Align::Auto => 0,
                Align::Center => self.view.rows / 2,
                Align::Bottom => self.view.rows - 1,
            };

            self.cur_line = self.top_line.clone();
            self.down_cur_line(rows)
        } else if pos < self.cur_line.row_pos {
            // pos is above cur row, but still visible, so find cur line, but keep
            // top line
            // find the new cur line which is needed for all cursor alignment scenarios
            let row = self.cursor.row - self.find_up_cur_line(pos);

            let maybe_rows = match align {
                Align::Auto => None,
                Align::Top => Some(0),
                Align::Center => Some(self.view.rows / 2),
                Align::Bottom => Some(self.view.rows - 1),
            };

            if let Some(rows) = maybe_rows {
                self.set_top_line(rows)
            } else {
                row
            }
        } else if pos < self.cur_line.end_pos() {
            // pos is already on cur row
            let maybe_rows = match align {
                Align::Auto => None,
                Align::Top => Some(0),
                Align::Center => Some(self.view.rows / 2),
                Align::Bottom => Some(self.view.rows - 1),
            };

            if let Some(rows) = maybe_rows {
                self.set_top_line(rows)
            } else {
                self.cursor.row
            }
        } else {
            // pos comes after cur row, so find cur line and then top line
            let rows = self.find_down_cur_line(pos);

            let row = match align {
                Align::Auto => cmp::min(self.cursor.row + rows, self.view.rows - 1),
                Align::Top => 0,
                Align::Center => self.view.rows / 2,
                Align::Bottom => self.view.rows - 1,
            };

            self.set_top_line(row)
        };

        let col = self.cur_line.col_of(pos);
        self.cur_pos = pos;
        self.snap_col = None;
        self.cursor = Point::new(row, col);
        self.render();
    }

    pub fn move_left(&mut self, len: usize) {
        let pos = self.cur_pos - cmp::min(len, self.cur_pos);
        if pos < self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    pub fn move_right(&mut self, len: usize) {
        let pos = cmp::min(self.cur_pos + len, self.buffer().size());
        if pos > self.cur_pos {
            self.move_to(pos, Align::Auto);
        }
    }

    pub fn move_page_up(&mut self) {
        self.move_up(self.view.rows, true);
    }

    pub fn move_page_down(&mut self) {
        self.move_down(self.view.rows, true);
    }

    /// Moves the cursor to the beginning of the current row.
    pub fn move_start(&mut self) {
        if self.cursor.col > 0 {
            self.cur_pos = self.cur_line.row_pos;
            self.cursor.col = 0;
            self.render();
        }
        self.snap_col = None;
    }

    /// Moves the cursor to the end of the current row.
    pub fn move_end(&mut self) {
        let end_col = cmp::min(self.cur_line.row_len as u32, self.view.cols - 1);
        if self.cursor.col < end_col {
            self.cur_pos = self.cur_line.pos_of(end_col);
            self.cursor.col = end_col;
            self.render();
        }
        self.snap_col = None;
    }

    /// Moves the cursor to the top of the buffer.
    pub fn move_top(&mut self) {
        self.move_to(0, Align::Top);
    }

    /// Moves the cursor to the bottom of the buffer.
    pub fn move_bottom(&mut self) {
        let pos = self.buffer().size();
        self.move_to(pos, Align::Bottom);
    }

    /// Scrolls the contents of the window up while preserving the cursor position, which
    /// means the cursor moves up as the contents scroll.
    ///
    /// If this operation would result in the cursor moving beyond the top row, then it
    /// is moved to the next row, essentially staying on the top row.
    pub fn scroll_up(&mut self, try_rows: u32) {
        let rows = self.down_top_line(try_rows);
        if rows > 0 {
            let (row, col) = if rows > self.cursor.row {
                // this means that scrolling would have pushed the cursor above the top
                // row, so set cur line to top line and row to 0
                self.cur_line = self.top_line.clone();
                let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
                let col = self.cur_line.snap_col(try_col);
                self.cur_pos = self.cur_line.pos_of(col);
                self.snap_col = Some(try_col);
                (0, col)
            } else {
                // this means that cursor still remains visible
                (self.cursor.row - rows, self.cursor.col)
            };

            self.cursor = Point::new(row, col);
            self.render();
        }
    }

    /// Scrolls the contents of the window down while preserving the cursor position, which
    /// means the cursor moves down as the contents scroll.
    ///
    /// If this operation would result in the cursor moving beyond the bottom row, then it
    /// is moved to the previous row, essentiall staying on the bottow row.
    pub fn scroll_down(&mut self, try_rows: u32) {
        let rows = self.up_top_line(try_rows);
        if rows > 0 {
            let row = self.cursor.row + rows;
            let (row, col) = if row < self.view.rows {
                // this means that cursor is still visible on display
                (row, self.cursor.col)
            } else {
                // this means that scrolling would have pushed the cursor below the bottom
                // back up from cur line to find the line at the bottom of the display
                self.up_cur_line(row - self.view.rows + 1);
                let try_col = self.snap_col.take().unwrap_or(self.cursor.col);
                let col = self.cur_line.snap_col(try_col);
                self.cur_pos = self.cur_line.pos_of(col);
                self.snap_col = Some(try_col);
                (self.view.rows - 1 as u32, col)
            };

            self.cursor = Point::new(row, col);
            self.render();
        }
    }

    fn render(&mut self) {
        let mut canvas = self.view.canvas.borrow_mut();
        let rest = self
            .buffer
            .borrow()
            .forward(self.top_line.row_pos)
            .try_fold((0, 0), |(row, col), c| {
                let (row, col) = if c == '\n' {
                    canvas.fill_row_from(row, col, self.view.blank_cell);
                    (row + 1, 0)
                } else {
                    canvas.set_cell(row, col, Cell::new(c, self.view.theme.text_color));
                    if col + 1 < self.view.cols {
                        (row, col + 1)
                    } else {
                        (row + 1, 0)
                    }
                };
                if row < self.view.rows {
                    Some((row, col))
                } else {
                    None
                }
            });
        if let Some((row, col)) = rest {
            canvas.fill_row_from(row, col, self.view.blank_cell);
            canvas.fill_rows(row + 1, self.view.rows, self.view.blank_cell);
        }
        canvas.draw();

        self.view
            .banner
            .borrow_mut()
            .set_cursor(self.cur_line.point_of(self.cursor.col))
            .draw();

        canvas.set_cursor(self.cursor);
    }

    // todo
    //
    // returns the line corresponding to the given pos
    fn find_line(&self, pos: usize) -> Line {
        let buffer = self.buffer.borrow();
        let line_pos = buffer.find_start_line(pos);
        let line_len = buffer.find_end_line(pos) - line_pos;
        let row_pos = pos - ((pos - line_pos) % self.view.cols as usize);
        let row_len = cmp::min(line_len - (row_pos - line_pos), self.view.cols as usize);
        Line {
            row_pos,
            row_len,
            line_pos,
            line_len,
            line_nbr: buffer.line_of(line_pos),
        }
    }

    // todo
    //
    // updates the line info and returns a new line
    fn update_line(&self, line: &Line) -> Line {
        // line_pos and row_pos would not change, only their lengths
        // line_nbr would also not change
        //
        // rationale for above is that an insertion is always relative to
        // cur_line, and even if the insertion happened at col 0, this does not
        // change any values above
        let buffer = self.buffer.borrow();
        let line_len = buffer.find_end_line(line.line_pos) - line.line_pos;
        let row_len = cmp::min(
            line_len - (line.row_pos - line.line_pos),
            self.view.cols as usize,
        );
        Line {
            row_len,
            line_len,
            ..*line
        }
    }

    // todo
    //
    // returns the previous line or None if the given line is at the top of buffer
    fn prev_line(&self, line: &Line) -> Option<Line> {
        if line.is_top() {
            None
        } else if line.has_wrapped() {
            let row_pos = line.row_pos - self.view.cols as usize;
            let l = Line {
                row_pos,
                row_len: self.view.cols as usize,
                ..*line
            };
            Some(l)
        } else {
            let buffer = self.buffer.borrow();
            let pos = line.line_pos - 1;
            let line_pos = buffer.find_start_line(pos);
            let line_len = buffer.find_end_line(pos) - line_pos;
            let row_pos = pos - ((pos - line_pos) % self.view.cols as usize);
            let row_len = cmp::min(line_len - (row_pos - line_pos), self.view.cols as usize);
            let l = Line {
                row_pos,
                row_len,
                line_pos,
                line_len,
                line_nbr: line.line_nbr - 1,
            };
            Some(l)
        }
    }

    fn prev_line_unchecked(&self, line: &Line) -> Line {
        self.prev_line(line)
            .unwrap_or_else(|| panic!("todo: add useful message"))
    }

    // todo
    //
    // returns the next line of None if the given line is at the bottom of buffer
    fn next_line(&self, line: &Line) -> Option<Line> {
        if line.row_len < self.view.cols as usize {
            // this means that row does not wrap, so we need to find next line
            let buffer = self.buffer.borrow();
            let line_pos = line.line_pos + line.line_len + 1;
            if line_pos <= buffer.size() {
                let line_len = buffer.find_end_line(line_pos) - line_pos;
                let row_pos = line_pos;
                let row_len = cmp::min(line_len - (row_pos - line_pos), self.view.cols as usize);
                let l = Line {
                    row_pos,
                    row_len,
                    line_pos,
                    line_len,
                    line_nbr: line.line_nbr + 1,
                };
                Some(l)
            } else {
                // end of buffer reached
                None
            }
        } else {
            // this means that the row wraps
            let row_pos = line.row_pos + line.row_len;
            let row_len = cmp::min(
                line.line_len - (row_pos - line.line_pos),
                self.view.cols as usize,
            );
            let l = Line {
                row_pos,
                row_len,
                ..*line
            };
            Some(l)
        }
    }

    fn next_line_unchecked(&self, line: &Line) -> Line {
        self.next_line(line)
            .unwrap_or_else(|| panic!("todo: add useful message"))
    }

    fn buffer(&self) -> Ref<'_, Buffer> {
        self.buffer.borrow()
    }

    fn buffer_mut(&self) -> RefMut<'_, Buffer> {
        self.buffer.borrow_mut()
    }

    fn canvas_mut(&self) -> RefMut<'_, Canvas> {
        self.view.canvas.borrow_mut()
    }
}
