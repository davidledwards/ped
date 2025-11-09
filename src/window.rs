//! A representation of a window.
//!
//! A window contains a _canvas_, which is the area comprised of editor text, and a
//! _banner_ for displaying other relevant bits of information.

use crate::canvas::{Canvas, CanvasRef};
use crate::color::Color;
use crate::config::ConfigurationRef;
use crate::size::{Point, Size};
use crate::source::Source;
use crate::sys;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

pub struct Window {
    size: Size,
    pub canvas: CanvasRef,
    pub banner: BannerRef,
}

pub type WindowRef = Rc<RefCell<Window>>;

pub struct Banner {
    canvas: Canvas,
    left_area: Option<Range<u32>>,
    right_area: Option<Range<u32>>,
    active_bg: u8,
    inactive_bg: u8,
    banner_color: Color,
    accent_color: Color,
    dirty_color: Color,
    dirty: bool,
    source: Source,
    syntax: String,
    eol: char,
    tab: char,
    ch: char,
    loc: Point,
}

pub type BannerRef = Rc<RefCell<Banner>>;

impl Banner {
    /// Minimum number of columns required to show _left_ area of banner.
    const MIN_COLS_FOR_LEFT: u32 = 16;

    /// Minimum number of columns required to show _left_ and _right_ areas of banner.
    const MIN_COLS_FOR_RIGHT: u32 = 40;

    /// Number of columns used as gap between distinct areas of banner.
    const GAP_COLS: u32 = 1;

    /// Prefix to use when truncating the source.
    const ELLIPSIS: &str = "...";

    /// Number of additional columns used to adorn syntax information.
    ///
    /// Layout is `(???)` where `???` is the variable-length syntax name.
    const SYNTAX_COLS: u32 = 2;

    /// Number of columns for mode settings.
    ///
    /// Layout is `-??-` where each `?` is a separate single-character indicator.
    const MODE_COLS: u32 = 4;

    /// Number of columns allocated to showing Unicode code point of character
    /// under cursor.
    ///
    /// Layout is `-????-` where '????` is the hex value of code point.
    const CHAR_COLS: u32 = 6;

    /// Number of columns allocated to line numbers.
    const LINE_COLS: u32 = 7;

    /// Maximum line number that can be shown based on allocated columns.
    const LINE_LIMIT: u32 = u32::pow(10, Self::LINE_COLS);

    /// Number of columns allocated to column numbers.
    const COL_COLS: u32 = 7;

    /// Maximum column number that can be shown based on allocated columns.
    const COL_LIMIT: u32 = u32::pow(10, Self::COL_COLS);

    /// Number of columns allocated to location area.
    const LOCATION_COLS: u32 = Self::LINE_COLS + Self::COL_COLS + 1;

    fn new(origin: Point, cols: u32, config: ConfigurationRef) -> Banner {
        // Determine which areas of banner will be shown based on available number of
        // columns.
        let (left_area, right_area) = Self::calc_areas(cols);
        let active_bg = config.theme.active_bg;
        let inactive_bg = config.theme.inactive_bg;
        let banner_color = Color::new(config.theme.banner_fg, inactive_bg);
        let accent_color = Color::new(config.theme.accent_fg, inactive_bg);
        let dirty_color = Color::new(config.theme.dirty_fg, inactive_bg);

        let mut this = Banner {
            canvas: Canvas::new(origin, Size::new(1, cols)),
            left_area,
            right_area,
            active_bg,
            inactive_bg,
            banner_color,
            accent_color,
            dirty_color,
            dirty: false,
            source: Source::Null,
            syntax: String::new(),
            eol: '?',
            tab: '?',
            ch: '\0',
            loc: Point::ORIGIN,
        };
        this.clear();
        this
    }

    pub fn none() -> Banner {
        Banner {
            canvas: Canvas::zero(),
            left_area: None,
            right_area: None,
            active_bg: 0,
            inactive_bg: 0,
            banner_color: Color::ZERO,
            accent_color: Color::ZERO,
            dirty_color: Color::ZERO,
            dirty: false,
            source: Source::Null,
            syntax: String::new(),
            eol: '?',
            tab: '?',
            ch: '\0',
            loc: Point::ORIGIN,
        }
    }

    /// Turns the banner into a [`BannerRef`].
    pub fn into_ref(self) -> BannerRef {
        Rc::new(RefCell::new(self))
    }

    /// Draws the banner bar by synchronizing pending changes.
    pub fn draw(&mut self) {
        self.canvas.draw();
    }

    /// Redraws the entire banner regardless of pending changes.
    pub fn redraw(&mut self) {
        self.clear();
        self.draw_left();
        self.draw_right();
        self.canvas.draw();
    }

    /// Changes the coloring of the banner bar based on its focus setting `yes`.
    pub fn focus(&mut self, yes: bool) {
        let bg = if yes {
            self.active_bg
        } else {
            self.inactive_bg
        };
        self.banner_color.bg = bg;
        self.accent_color.bg = bg;
        self.dirty_color.bg = bg;
        self.redraw();
    }

    pub fn set_dirty(&mut self, dirty: bool) -> &mut Banner {
        if dirty != self.dirty {
            self.dirty = dirty;
            self.draw_left();
        }
        self
    }

    pub fn set_source(&mut self, source: Source) -> &mut Banner {
        self.source = source;
        self.draw_left();
        self
    }

    pub fn set_syntax(&mut self, syntax: String) -> &mut Banner {
        self.syntax = syntax;
        self.draw_left();
        self
    }

    pub fn set_eol(&mut self, crlf: bool) -> &mut Banner {
        self.eol = if crlf { '\\' } else { '/' };
        self.draw_left();
        self
    }

    pub fn set_tab(&mut self, tab_hard: bool) -> &mut Banner {
        self.tab = if tab_hard { 'T' } else { 't' };
        self.draw_left();
        self
    }

    pub fn set_char(&mut self, c: Option<char>) -> &mut Banner {
        self.ch = c.unwrap_or('\0');
        self.draw_right();
        self
    }

    pub fn set_location(&mut self, loc: Point) -> &mut Banner {
        self.loc = loc;
        self.draw_right();
        self
    }

    fn clear(&mut self) {
        self.canvas.fill_row(0, ' ', self.banner_color);
    }

    #[rustfmt::skip]
    fn draw_left(&mut self) {
        if let Some(Range { start, end }) = self.left_area {
            // Mode information requires fixed amount of space, so deduct this amount
            // and standard gap from available space.
            let avail_cols =
                end - start        // full range
                - Self::GAP_COLS   // gap between source and mode
                - Self::MODE_COLS; // mode area

            let mut source = self.source.to_string().chars().collect::<Vec<_>>();
            let mut syntax = self.syntax.chars().collect::<Vec<_>>();

            // Calculate needed number of columns for both source and syntax.
            let need_cols =
                source.len() as u32   // source
                + Self::GAP_COLS      // gap between source and syntax
                + syntax.len() as u32 // syntax
                + Self::SYNTAX_COLS;  // `(` and `)` around syntax

            if need_cols > avail_cols {
                // Try shortening source by using file name portion only, though note
                // that shortening may not actually happen.
                if let Source::File(path, _) = &self.source {
                    source = sys::file_name(path).chars().collect::<Vec<_>>();
                }

                let need_cols =
                    source.len() as u32   // source
                    + Self::GAP_COLS      // gap between source and syntax
                    + syntax.len() as u32 // syntax
                    + Self::SYNTAX_COLS;  // `(` and `)` around syntax

                if need_cols > avail_cols {
                    // Try clipping syntax information as next attempt to fit within
                    // available area.
                    syntax.clear();

                    if source.len() as u32 > avail_cols {
                        // Final attempt truncates prefix of source, but adds ellipsis as
                        // visual cue that truncation occurred.
                        source.drain(0..source.len() - avail_cols as usize + Self::ELLIPSIS.len());
                        source.splice(0..0, Self::ELLIPSIS.chars());
                    }
                }
            }

            // Draw possibly clipped forms of source and syntax on canvas.
            let mut col = start;
            col += self.canvas.write(
                0,
                col,
                &source,
                if self.dirty {
                    self.dirty_color
                } else {
                    self.banner_color
                },
            );
            if syntax.len() > 0 {
                col += self.canvas.write_str(0, col, " (", self.banner_color);
                col += self.canvas.write(0, col, &syntax, self.accent_color);
                col += self.canvas.write_char(0, col, ')', self.banner_color);
            }

            // Draw mode information.
            col += self.canvas.write_str(0, col, " -", self.banner_color);
            col += self.canvas.write_char(0, col, self.eol, self.accent_color);
            col += self.canvas.write_char(0, col, self.tab, self.accent_color);
            col += self.canvas.write_char(0, col, '-', self.banner_color);

            // Clear any remaining space.
            self.canvas.fill(0, col..end, ' ', self.banner_color);
        }
    }

    #[rustfmt::skip]
    fn draw_right(&mut self) {
        if let Some(Range { start, end }) = self.right_area {
            // Draw character under cursor as hex code point, which is anchored at
            // leftmost position in right area.
            let mut col = start;
            col += self.canvas.write_char(0, col, '-', self.banner_color);
            col += self.canvas.write_str(
                0,
                col,
                &format!("{:04x}", self.ch as u32),
                self.accent_color,
            );
            col += self.canvas.write_char(0, col, '-', self.banner_color);

            // Format line and column numbers, both of which might be shown as dashes
            // if values are too large to fit within bounds of available area. Locations
            // always displayed as 1-based, hence adjustment.
            let loc = self.loc + (1, 1);
            let line_str = if loc.row < Self::LINE_LIMIT {
                format!("{}", loc.row)
            } else {
                "-".repeat(Self::LINE_COLS as usize)
            };
            let col_str = if loc.col < Self::COL_LIMIT {
                format!("{}", loc.col)
            } else {
                "-".repeat(Self::COL_COLS as usize)
            };

            // Since location is right-justified, draw any necessary whitespace first.
            let next_col = end
                - Self::GAP_COLS        // gap between code point and location
                - line_str.len() as u32 // line number
                - 1                     // `:`
                - col_str.len() as u32; // column number
            self.canvas.fill(0, col..next_col, ' ', self.banner_color);

            // Draw line and column.
            col = next_col;
            col += self.canvas.write_str(0, col, &line_str, self.banner_color);
            col += self.canvas.write_char(0, col, ':', self.accent_color);
            self.canvas.write_str(0, col, &col_str, self.banner_color);
        }
    }

    /// Returns column ranges allocated to _left_ and _right_ areas of banner bar,
    /// respectively.
    #[rustfmt::skip]
    fn calc_areas(cols: u32) -> (Option<Range<u32>>, Option<Range<u32>>) {
        if cols < Self::MIN_COLS_FOR_LEFT {
            (None, None)
        } else if cols < Self::MIN_COLS_FOR_RIGHT {
            let left_area =
                Self::GAP_COLS        // left margin
                ..cols
                - Self::GAP_COLS;     // right margin
            (Some(left_area), None)
        } else {
            let left_area =
                Self::GAP_COLS        // left margin
                ..cols
                - Self::GAP_COLS      // right margin
                - Self::LOCATION_COLS // location area
                - Self::GAP_COLS      // gap between code point and location
                - Self::CHAR_COLS     // code point area
                - Self::GAP_COLS;     // gap betewen left and right areas
            let right_area =
                cols
                - Self::GAP_COLS      // right margin
                - Self::LOCATION_COLS // location area
                - Self::GAP_COLS      // gap between code point and location
                - Self::CHAR_COLS     // code point area
                ..cols
                    - Self::GAP_COLS; // right margin
            (Some(left_area), Some(right_area))
        }
    }
}

impl Window {
    const CANVAS_ORIGIN_OFFSET: Size = Size::ZERO;
    const CANVAS_SIZE_ADJUST: Size = Size::rows(1);

    pub fn new(origin: Point, size: Size, config: ConfigurationRef) -> Window {
        let canvas = Canvas::new(
            origin + Self::CANVAS_ORIGIN_OFFSET,
            size - Self::CANVAS_SIZE_ADJUST,
        );
        let banner = Banner::new(
            origin + Size::rows(size.rows - 1),
            size.cols,
            config.clone(),
        );
        let mut this = Window {
            size,
            canvas: canvas.into_ref(),
            banner: banner.into_ref(),
        };
        this.draw();
        this
    }

    pub fn zombie() -> Window {
        Window {
            size: Size::ZERO,
            canvas: Canvas::zero().into_ref(),
            banner: Banner::none().into_ref(),
        }
    }

    pub fn is_zombie(&self) -> bool {
        self.size == Size::ZERO
    }

    pub fn into_ref(self) -> WindowRef {
        Rc::new(RefCell::new(self))
    }

    pub fn draw(&mut self) {
        self.banner.borrow_mut().draw();
    }

    /// Returns the point relative to the window canvas corresponding to `p`, which
    /// is a point presumed to be relative to the top-left position of the terminal
    /// display, or `None` if `p` is not contained within the canvas area.
    pub fn point_on_canvas(&self, p: Point) -> Option<Point> {
        let (origin, size) = {
            let canvas = self.canvas.borrow();
            (canvas.origin(), canvas.size())
        };
        if p.row >= origin.row
            && p.row < origin.row + size.rows
            && p.col >= origin.col
            && p.col < origin.col + size.cols
        {
            Some(p - origin)
        } else {
            None
        }
    }
}
