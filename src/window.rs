//! Window management.
use crate::canvas::{Canvas, CanvasRef};
use crate::config::{Configuration, ConfigurationRef};
use crate::size::{Point, Size};
use crate::writer::Writer;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Banner {
    cols: u32,
    config: ConfigurationRef,
    title: String,
    dirty: bool,
    loc: Point,
    writer: Writer,
}

pub type BannerRef = Rc<RefCell<Banner>>;

impl Banner {
    /// Any window less than this width means the banner contents are not drawn.
    const MIN_COLS: u32 = 16;

    /// Total number of columns used for whitespace under normal circumstances.
    /// - 2: left edge
    /// - 2: gap between title and location
    /// - 2: right edge
    const WS_SIZE: usize = 6;

    /// Number of columns for whitespace between title and location.
    const GAP_SIZE: usize = 2;

    fn new(origin: Point, cols: u32, config: ConfigurationRef) -> Banner {
        Banner {
            cols,
            config,
            title: String::new(),
            dirty: false,
            loc: Point::ORIGIN,
            writer: Writer::new(origin),
        }
    }

    pub fn none() -> Banner {
        Banner {
            cols: 0,
            config: Configuration::default().to_ref(),
            title: String::new(),
            dirty: false,
            loc: Point::ORIGIN,
            writer: Writer::new(Point::ORIGIN),
        }
    }

    /// Turns the banner into a [`BannerRef`].
    pub fn to_ref(self) -> BannerRef {
        Rc::new(RefCell::new(self))
    }

    pub fn draw(&mut self) {
        self.writer
            .set_origin()
            .set_color(self.config.colors.banner);

        if self.cols < Self::MIN_COLS {
            self.writer
                .write_str(" ".repeat(self.cols as usize).as_str());
        } else {
            // Locations are always displayed to users as 1-based.
            let loc = format!("{}:{}", self.loc.row + 1, self.loc.col + 1);

            // Calculates number of columns required to display full content of
            // banner.
            let len = self.title.len() + loc.len() + Self::WS_SIZE;

            // Clip banner contents if required number of columns exceeds width
            // of window.
            let (title, loc) = if len > self.cols as usize {
                // Prioritize removal of location information, which also means that
                // gap between title and location can be reclaimed as well.
                let len = len - loc.len() - Self::GAP_SIZE;

                // Truncate title if necessary.
                let title = if len > self.cols as usize {
                    &self.title.as_str()[..(self.title.len() - (len - self.cols as usize))]
                } else {
                    self.title.as_str()
                };
                (title, "".to_string())
            } else {
                (self.title.as_str(), loc)
            };

            // Calculate number of columns needed for gap between title and location
            // based on clipped content.
            let gap_len =
                self.cols as usize - title.len() - loc.len() - (Self::WS_SIZE - Self::GAP_SIZE);

            self.writer
                .write(' ')
                .write(if self.dirty { '*' } else { ' ' })
                .write_str(title)
                .write_str(" ".repeat(gap_len).as_str())
                .write_str(loc.as_str())
                .write_str("  ");
        }
        self.writer.send();
    }

    pub fn set_title(&mut self, title: String) -> &mut Banner {
        self.title = title;
        self
    }

    pub fn set_dirty(&mut self, dirty: bool) -> &mut Banner {
        self.dirty = dirty;
        self
    }

    pub fn set_location(&mut self, loc: Point) -> &mut Banner {
        self.loc = loc;
        self
    }
}

pub struct Window {
    size: Size,
    config: ConfigurationRef,
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
            config,
            canvas: canvas.to_ref(),
            banner: banner.to_ref(),
        };
        this.draw();
        this
    }

    pub fn zombie() -> Window {
        Window {
            size: Size::ZERO,
            config: Configuration::default().to_ref(),
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

    pub fn config(&self) -> &ConfigurationRef {
        &self.config
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
}
