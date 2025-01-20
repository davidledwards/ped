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
use std::usize;

pub struct Banner {
    canvas: Canvas,
    dirty_area: Option<u32>,
    source_area: Option<Range<u32>>,
    loc_area: Option<Range<u32>>,
    banner_color: Color,
    accent_color: Color,
    dirty: bool,
    source: Source,
    syntax: String,
    loc: Point,
}

pub type BannerRef = Rc<RefCell<Banner>>;

impl Banner {
    /// Minimum number of window columns required to show any banner information,
    /// otherwise everything is clipped.
    const MIN_COLS: u32 = 16;

    /// Minimum number of window columns required to show location information,
    /// otherwise it is clipped.
    const MIN_COLS_FOR_LOCATION: u32 = 32;

    /// Number of columns allocated to left margin.
    const LEFT_MARGIN_COLS: u32 = 1;

    /// Number of columns allocated to right margin.
    const RIGHT_MARGIN_COLS: u32 = 2;

    /// Prefix to use when truncating the source.
    const SOURCE_ELLIPSIS: &str = "...";

    /// Number of additional columns required in the source and syntax area for
    /// whitespace and other adornments.
    const SOURCE_ADORN_COLS: usize = 3;

    /// Number of columns allocated to whitespace between source and location areas.
    const GAP_COLS: u32 = 2;

    /// Number of columns allocated to line numbers.
    const LINE_COLS: u32 = 5;

    /// Maximum line number that can be shown based on allocated columns.
    const LINE_LIMIT: u32 = u32::pow(10, Self::LINE_COLS);

    /// Number of columns allocated to column numbers.
    const COL_COLS: u32 = 4;

    /// Maximum column number that can be shown based on allocated columns.
    const COL_LIMIT: u32 = u32::pow(10, Self::COL_COLS);

    /// Number of columns allocated to location area.
    const LOCATION_COLS: u32 = Self::LINE_COLS + Self::COL_COLS + 1;

    fn new(origin: Point, cols: u32, config: ConfigurationRef) -> Banner {
        // Determine which areas of banner will be shown based on available number of
        // columns.
        let (dirty_area, source_area, loc_area) = Self::calc_areas(cols);
        let banner_color = Color::new(config.theme.banner_fg, config.theme.banner_bg);
        let accent_color = Color::new(config.theme.accent_fg, config.theme.banner_bg);

        // Initialize entire canvas with blanks.
        let mut canvas = Canvas::new(origin, Size::new(1, cols));
        canvas.fill_row(0, ' ', banner_color);

        Banner {
            canvas,
            dirty_area,
            source_area,
            loc_area,
            banner_color,
            accent_color,
            dirty: false,
            source: Source::Null,
            syntax: String::new(),
            loc: Point::ORIGIN,
        }
    }

    pub fn none() -> Banner {
        Banner {
            canvas: Canvas::zero(),
            dirty_area: None,
            source_area: None,
            loc_area: None,
            banner_color: Color::ZERO,
            accent_color: Color::ZERO,
            dirty: false,
            source: Source::Null,
            syntax: String::new(),
            loc: Point::ORIGIN,
        }
    }

    /// Turns the banner into a [`BannerRef`].
    pub fn to_ref(self) -> BannerRef {
        Rc::new(RefCell::new(self))
    }

    pub fn draw(&mut self) {
        self.canvas.draw();
    }

    pub fn set_dirty(&mut self, dirty: bool) -> &mut Banner {
        if dirty != self.dirty {
            self.dirty = dirty;
            self.draw_dirty();
        }
        self
    }

    pub fn set_source(&mut self, source: Source) -> &mut Banner {
        self.source = source;
        self.draw_source();
        self
    }

    pub fn set_syntax(&mut self, syntax: String) -> &mut Banner {
        self.syntax = syntax;
        self.draw_source();
        self
    }

    pub fn set_location(&mut self, loc: Point) -> &mut Banner {
        self.loc = loc;
        self.draw_location();
        self
    }

    fn draw_dirty(&mut self) {
        if let Some(col) = self.dirty_area {
            let c = if self.dirty { '*' } else { ' ' };
            self.canvas.set(0, col, c, self.accent_color);
        }
    }

    fn draw_source(&mut self) {
        if let Some(Range { start, end }) = self.source_area {
            let avail_cols = (end - start) as usize;
            let mut source = self.source.to_string().chars().collect::<Vec<_>>();
            let mut syntax = self.syntax.chars().collect::<Vec<_>>();

            if source.len() + syntax.len() + Self::SOURCE_ADORN_COLS > avail_cols {
                // Try shortening source by using file name portion only, though note
                // that shortening may not actually happen.
                if let Source::File(path, _) = &self.source {
                    source = sys::file_name(path).chars().collect::<Vec<_>>();
                }

                if source.len() + syntax.len() + Self::SOURCE_ADORN_COLS > avail_cols {
                    // Try clipping syntax information as next attempt to fit within
                    // available area.
                    syntax.clear();

                    if source.len() > avail_cols {
                        // Final attempt truncates prefix of source, but adds ellipsis as
                        // visual cue that truncation occurred.
                        source.drain(0..source.len() - avail_cols + Self::SOURCE_ELLIPSIS.len());
                        source.splice(0..0, Self::SOURCE_ELLIPSIS.chars());
                    }
                }
            }

            // Draw possibly clipped forms of source and syntax on canvas.
            let mut col = start;
            col += self.canvas.write(0, col, &source, self.banner_color);
            if syntax.len() > 0 {
                col += self.canvas.write_str(0, col, " (", self.banner_color);
                col += self.canvas.write(0, col, &syntax, self.accent_color);
                col += self.canvas.write_char(0, col, ')', self.banner_color);
            }
            self.canvas.fill(0, col..end, ' ', self.banner_color);
        }
    }

    fn draw_location(&mut self) {
        if let Some(Range { start, end }) = self.loc_area {
            // Locations always displayed as 1-based, hence adjustment.
            let loc = self.loc + (1, 1);

            // Format line and column numbers, both of which might be shown as dashes
            // if values are too large to fit within bounds of available area.
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

            // Since line and column is displayed right-justified, draw any necessary
            // whitespace first.
            let n = line_str.len() + col_str.len() + 1;
            let mut col = end - n as u32;
            self.canvas.fill(0, start..col, ' ', self.banner_color);
            col += self.canvas.write_str(0, col, &line_str, self.banner_color);
            col += self.canvas.write_char(0, col, ':', self.accent_color);
            self.canvas.write_str(0, col, &col_str, self.banner_color);
        }
    }

    fn calc_areas(cols: u32) -> (Option<u32>, Option<Range<u32>>, Option<Range<u32>>) {
        if cols < Self::MIN_COLS {
            (None, None, None)
        } else if cols < Self::MIN_COLS_FOR_LOCATION {
            // Clip location area entirely, which increases area for source and syntax.
            let dirty_area = Self::LEFT_MARGIN_COLS;
            let source_area = dirty_area + 1..cols - Self::RIGHT_MARGIN_COLS;
            (Some(dirty_area), Some(source_area), None)
        } else {
            // Limit area available for source and syntax.
            let dirty_area = Self::LEFT_MARGIN_COLS;
            let source_area = dirty_area + 1
                ..cols - Self::RIGHT_MARGIN_COLS - Self::LOCATION_COLS - Self::GAP_COLS;
            let loc_area = cols - Self::RIGHT_MARGIN_COLS - Self::LOCATION_COLS
                ..cols - Self::RIGHT_MARGIN_COLS;
            (Some(dirty_area), Some(source_area), Some(loc_area))
        }
    }
}

pub struct Window {
    size: Size,
    canvas: CanvasRef,
    banner: BannerRef,
}

pub type WindowRef = Rc<RefCell<Window>>;

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
            canvas: canvas.to_ref(),
            banner: banner.to_ref(),
        };
        this.draw();
        this
    }

    pub fn zombie() -> Window {
        Window {
            size: Size::ZERO,
            canvas: Canvas::zero().to_ref(),
            banner: Banner::none().to_ref(),
        }
    }

    pub fn is_zombie(&self) -> bool {
        self.size == Size::ZERO
    }

    pub fn to_ref(self) -> WindowRef {
        Rc::new(RefCell::new(self))
    }

    pub fn canvas(&self) -> &CanvasRef {
        &self.canvas
    }

    pub fn banner(&self) -> &BannerRef {
        &self.banner
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
