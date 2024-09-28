//! Window management.
use crate::canvas::{Canvas, CanvasRef};
use crate::color::Color;
use crate::display::{Display, Point, Size};

use std::cell::RefCell;
use std::rc::Rc;

pub struct Window {
    origin: Point,
    size: Size,
    canvas: CanvasRef,
}

pub type WindowRef = Rc<RefCell<Window>>;

impl Window {
    const CANVAS_ORIGIN_OFFSET: Size = Size::new(0, 0);
    const CANVAS_SIZE_ADJUST: Size = Size::new(1, 0);

    pub fn new(origin: Point, size: Size, color: Color) -> Window {
        let canvas_origin = origin + Self::CANVAS_ORIGIN_OFFSET;
        let canvas_size = size - Self::CANVAS_SIZE_ADJUST;
        let canvas = Canvas::new(canvas_origin, canvas_size, color);

        // temp for now, just showing border to verify working
        Display::new(origin)
            .set_cursor(Point::new(size.rows - 1, 0))
            .set_color(Color::new(233, 15))
            .write_str("-".repeat(size.cols as usize).as_str())
            .send();

        Window {
            origin,
            size,
            canvas: Canvas::to_ref(canvas),
        }
    }

    pub fn to_ref(window: Window) -> WindowRef {
        Rc::new(RefCell::new(window))
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn canvas(&self) -> &CanvasRef {
        &self.canvas
    }

    pub fn draw(&mut self) {
        // draw window border
        // draw canvas
    }
}
