//! Window management.
use crate::canvas::{Canvas, CanvasRef};
use crate::color::Color;
use crate::display::{Display, Point, Size};

use std::cell::RefCell;
use std::rc::Rc;

pub struct Banner {
    origin: Point,
    cols: u32,
    title: String,
    dirty: bool,
    cursor: Point,
    display: Display,
}

pub type BannerRef = Rc<RefCell<Banner>>;

impl Banner {
    /// Any window less than this width means the banner contents are not drawn.
    const MIN_COLS: u32 = 16;

    /// Total number of columns used for whitespace under normal circumstances.
    /// - 2: left edge
    /// - 2: gap between title and cursor
    /// - 2: right edge
    const WS_SIZE: usize = 6;

    /// Number of columns for whitespace between title and cursor.
    const GAP_SIZE: usize = 2;

    fn new(origin: Point, cols: u32) -> Banner {
        Banner {
            origin,
            cols,
            title: String::new(),
            dirty: false,
            cursor: Point::ORIGIN,
            display: Display::new(origin),
        }
    }

    pub fn none() -> Banner {
        Banner {
            origin: Point::ORIGIN,
            cols: 0,
            title: String::new(),
            dirty: false,
            cursor: Point::ORIGIN,
            display: Display::new(Point::ORIGIN),
        }
    }

    /// Turns the banner into a [`BannerRef`].
    pub fn to_ref(self: Banner) -> BannerRef {
        Rc::new(RefCell::new(self))
    }

    pub fn draw(&mut self) {
        self.display
            .set_cursor(Point::ORIGIN)
            .set_color(Color::new(232, 253));

        if self.cols < Self::MIN_COLS {
            self.display
                .write_str(" ".repeat(self.cols as usize).as_str());
        } else {
            // Calculates number of columns required to display full content of
            // banner.
            let cursor = format!("{}", self.cursor);
            let len = self.title.len() + cursor.len() + Self::WS_SIZE;

            // Clip banner contents if required number of columns exceeds width
            // of window.
            let (title, cursor) = if len > self.cols as usize {
                // Prioritize removal of cursor information, which also means that
                // gap between title and cursor can be reclaimed as well.
                let len = len - cursor.len() - Self::GAP_SIZE;

                // Truncate title if necessary.
                let title = if len > self.cols as usize {
                    &self.title.as_str()[..(self.title.len() - (len - self.cols as usize))]
                } else {
                    self.title.as_str()
                };
                (title, "".to_string())
            } else {
                (self.title.as_str(), cursor)
            };

            // Calculate number of columns needed for gap between title and cursor
            // based on clipped content.
            let gap_len =
                self.cols as usize - title.len() - cursor.len() - (Self::WS_SIZE - Self::GAP_SIZE);

            self.display
                .write(' ')
                .write(if self.dirty { '*' } else { ' ' })
                .write_str(title)
                .write_str(" ".repeat(gap_len).as_str())
                .write_str(cursor.as_str())
                .write_str("  ");
        }
        self.display.send();
    }

    pub fn set_title(&mut self, title: String) -> &mut Banner {
        self.title = title;
        self
    }

    pub fn set_dirty(&mut self, dirty: bool) -> &mut Banner {
        self.dirty = dirty;
        self
    }

    pub fn set_cursor(&mut self, cursor: Point) -> &mut Banner {
        self.cursor = cursor;
        self
    }
}

pub struct Window {
    origin: Point,
    size: Size,
    canvas: CanvasRef,
    banner: BannerRef,
}

pub type WindowRef = Rc<RefCell<Window>>;

impl Window {
    const CANVAS_ORIGIN_OFFSET: Size = Size::new(0, 0);
    const CANVAS_SIZE_ADJUST: Size = Size::new(1, 0);

    pub fn new(origin: Point, size: Size, color: Color) -> Window {
        let canvas_origin = origin + Self::CANVAS_ORIGIN_OFFSET;
        let canvas_size = size - Self::CANVAS_SIZE_ADJUST;
        let canvas = Canvas::new(canvas_origin, canvas_size, color);
        let banner = Banner::new(origin + (size.rows - 1, 0), size.cols);

        let mut this = Window {
            origin,
            size,
            canvas: canvas.to_ref(),
            banner: banner.to_ref(),
        };
        this.draw();
        this
    }

    pub fn zombie() -> Window {
        Window {
            origin: Point::ORIGIN,
            size: Size::ZERO,
            canvas: Canvas::zero().to_ref(),
            banner: Banner::none().to_ref(),
        }
    }

    pub fn is_zombie(&self) -> bool {
        self.size == Size::ZERO
    }

    pub fn to_ref(self: Window) -> WindowRef {
        Rc::new(RefCell::new(self))
    }

    pub fn size(&self) -> Size {
        self.size
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
