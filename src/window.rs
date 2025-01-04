//! A representation of a window.
//!
//! A window contains a *canvas*, which is the area comprised of editor text, and a
//! *banner* for displaying other relevant bits of information.

use crate::canvas::{Canvas, CanvasRef};
use crate::config::{Configuration, ConfigurationRef};
use crate::size::{Point, Size};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::usize;

pub struct Banner {
    config: ConfigurationRef,
    canvas: Canvas,
    dirty_area: Option<u32>,
    title_area: Option<Range<u32>>,
    loc_area: Option<Range<u32>>,
    dirty: bool,
    title: String,
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

    /// Number of columns allocated to left and right margins.
    const MARGIN_COLS: u32 = 1;

    /// Number of additional columns required in the title and syntax area for
    /// whitespace and other adornments.
    const TITLE_ADORN_COLS: u32 = 3;

    /// Number of columns allocated to whitespace between title and location areas.
    const GAP_COLS: u32 = 2;

    /// Number of columns allocated to line numbers.
    const LINE_COLS: u32 = 5;

    /// Maximum line number that can be shown based on allocated columns.
    const LINE_LIMIT: u32 = u32::pow(10, Self::LINE_COLS - 1);

    /// Number of columns allocated to column numbers.
    const COL_COLS: u32 = 4;

    /// Maximum column number that can be shown based on allocated columns.
    const COL_LIMIT: u32 = u32::pow(10, Self::COL_COLS - 1);

    /// Number of columns allocated to location area.
    const LOCATION_COLS: u32 = Self::LINE_COLS + Self::COL_COLS + 1;

    fn new(origin: Point, cols: u32, config: ConfigurationRef) -> Banner {
        // Initialize entire canvas with blanks.
        let mut canvas = Canvas::new(origin, Size::new(1, cols));
        canvas.fill_row(0, ' ', config.theme.banner_color);

        // Determine which areas of banner will be shown based on available number of
        // columns.
        let (dirty_area, title_area, loc_area) = Self::calc_areas(cols);

        Banner {
            config,
            canvas,
            dirty_area,
            title_area,
            loc_area,
            dirty: false,
            title: String::new(),
            syntax: String::new(),
            loc: Point::ORIGIN,
        }
    }

    pub fn none() -> Banner {
        Banner {
            config: Configuration::default().to_ref(),
            canvas: Canvas::zero(),
            dirty_area: None,
            title_area: None,
            loc_area: None,
            dirty: false,
            title: String::new(),
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

    pub fn set_title(&mut self, title: String) -> &mut Banner {
        if title != self.title {
            self.title = title;
            self.draw_title();
        }
        self
    }

    pub fn set_syntax(&mut self, syntax: String) -> &mut Banner {
        if syntax != self.syntax {
            self.syntax = syntax;
            self.draw_title();
        }
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
            self.canvas
                .write_char(0, col, c, self.config.theme.banner_color);
        }
    }

    fn draw_title(&mut self) {
        if let Some(Range { start, end }) = self.title_area {
            let avail_cols = (end - start) as usize;
            let mut title_chars = self.title.chars().collect::<Vec<_>>();
            let mut syntax_chars = self.syntax.chars().collect::<Vec<_>>();

            // Prioritize clipping of syntax before trimming prefix of title to fit
            // within bounds of available area.
            let need_cols =
                title_chars.len() + syntax_chars.len() + Self::TITLE_ADORN_COLS as usize;

            if need_cols > avail_cols {
                syntax_chars.clear();
                if title_chars.len() > avail_cols {
                    title_chars.drain(0..title_chars.len() - avail_cols);
                }
            }

            // Draw possibly clipped forms of title and syntax on canvas.
            let color = self.config.theme.banner_color;
            let mut col = start;
            col += self.canvas.write(0, col, &title_chars, color);
            if syntax_chars.len() > 0 {
                col += self.canvas.write_str(0, col, " (", color);
                col += self.canvas.write(0, col, &syntax_chars, color);
                col += self.canvas.write_char(0, col, ')', color);
            }
            self.canvas.fill(0, col..end, ' ', color);
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
            let color = self.config.theme.banner_color;
            let n = line_str.len() + col_str.len() + 1;
            let mut col = end - n as u32;
            self.canvas.fill(0, start..col, ' ', color);
            col += self.canvas.write_str(0, col, &line_str, color);
            col += self.canvas.write_char(0, col, ':', color);
            self.canvas.write_str(0, col, &col_str, color);
        }
    }

    fn calc_areas(cols: u32) -> (Option<u32>, Option<Range<u32>>, Option<Range<u32>>) {
        if cols < Self::MIN_COLS {
            (None, None, None)
        } else if cols < Self::MIN_COLS_FOR_LOCATION {
            // Clip location area entirely, which increases area for title and syntax.
            let dirty_area = Self::MARGIN_COLS;
            let title_area = dirty_area + 1..cols - Self::MARGIN_COLS;
            (Some(dirty_area), Some(title_area), None)
        } else {
            // Limit area available for title and syntax.
            let dirty_area = Self::MARGIN_COLS;
            let title_area =
                dirty_area + 1..cols - Self::MARGIN_COLS - Self::LOCATION_COLS - Self::GAP_COLS;
            let loc_area = cols - Self::MARGIN_COLS - Self::LOCATION_COLS..cols - Self::MARGIN_COLS;
            (Some(dirty_area), Some(title_area), Some(loc_area))
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
        let canvas_origin = origin + Self::CANVAS_ORIGIN_OFFSET;
        let canvas_size = size - Self::CANVAS_SIZE_ADJUST;
        let banner_origin = origin + Size::rows(size.rows - 1);
        let canvas = Canvas::new(canvas_origin, canvas_size);
        let banner = Banner::new(banner_origin, size.cols, config.clone());

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
