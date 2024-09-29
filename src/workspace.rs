//! Workspace management.
use crate::color::Color;
use crate::display::{Point, Size};
use crate::error::Result;
use crate::window::{Window, WindowRef};

#[derive(Debug)]
pub enum Placement {
    Top,
    Bottom,
    Above(u32),
    Below(u32),
}

pub struct View {
    id: u32,
    origin: Point,
    size: Size,
    window: WindowRef,
}

impl View {
    fn new(id: u32, origin: Point, size: Size, window: Window) -> View {
        View {
            id,
            origin,
            size,
            window: window.to_ref(),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn origin(&self) -> Point {
        self.origin
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn window(&self) -> &WindowRef {
        &self.window
    }
}

pub struct Views<'a> {
    views: &'a Vec<View>,
    index: usize,
}

impl Views<'_> {
    fn new(ws: &Workspace) -> Views {
        Views {
            views: &ws.views,
            index: 0,
        }
    }
}

impl<'a> Iterator for Views<'a> {
    type Item = &'a View;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.views.len() {
            let r = &self.views[self.index];
            self.index += 1;
            Some(r)
        } else {
            None
        }
    }
}

pub struct Workspace {
    origin: Point,
    size: Size,
    view_origin: Point,
    view_size: Size,
    id_seq: u32,
    views: Vec<View>,
}

impl Workspace {
    const VIEW_ORIGIN_OFFSET: Size = Size::new(0, 0);
    const VIEW_SIZE_ADJUST: Size = Size::new(1, 0);
    const MIN_ROWS: u32 = 3;

    pub fn new(origin: Point, size: Size) -> Result<Workspace> {
        let view_origin = origin + Self::VIEW_ORIGIN_OFFSET;
        let view_size = size - Self::VIEW_SIZE_ADJUST;
        let mut this = Workspace {
            origin,
            size,
            view_origin,
            view_size,
            id_seq: 0,
            views: vec![],
        };
        this.add_view(Placement::Top);
        Ok(this)
    }

    pub fn default_view(&self) -> &View {
        self.views
            .get(0)
            .unwrap_or_else(|| panic!("invariant violation: > 0 views must exist"))
    }

    pub fn add_view(&mut self, place: Placement) -> Option<&View> {
        // Calculate effective number of rows to be allocated to each view given addition
        // of new view, which must satisfy MIN_ROWS requirement, otherwise operation is
        // rejected.
        let view_count = self.views.len() + 1;
        let view_rows = self.view_size.rows / view_count as u32;

        if view_rows < Self::MIN_ROWS {
            None
        } else {
            // Generate unique id for new view.
            let view_id = self.next_id();

            // Find correct index for insertion of new window.
            let index = match place {
                Placement::Top => 0,
                Placement::Bottom => view_count - 1,
                Placement::Above(id) => self
                    .views
                    .iter()
                    .position(|v| v.id == id)
                    .unwrap_or_else(|| panic!("{place:?}: id not found")),
                Placement::Below(id) => self
                    .views
                    .iter()
                    .position(|v| v.id == id)
                    .map(|i| i + 1)
                    .unwrap_or_else(|| panic!("{place:?}: id not found")),
            };

            // Since views will not necessarily occupy same number of rows since division
            // could be fractional, capturing remainder as number of residual rows allows
            // us to allocate them to some views.
            let residual_rows = self.view_size.rows % view_count as u32;

            // Recreate views based on addition of new window, essentially moving left to
            // right through each view (effectively, top to bottom starting at view origin)
            // and resizes based on newly calculated number of rows.
            let (views, _) =
                (0..view_count).fold((Vec::new(), self.view_origin), |(mut views, origin), i| {
                    // Select id to use based on index.
                    let id = if i == index {
                        view_id
                    } else {
                        self.views[if i < index { i } else { i - 1 }].id
                    };

                    // Give preference of residual rows to top-most windows.
                    let rows = if i >= residual_rows as usize {
                        view_rows
                    } else {
                        view_rows + 1
                    };

                    // Create view with new origin and size.
                    let size = Size::new(rows, self.view_size.cols);
                    let window = Window::new(origin, size, Color::new(15, 233));
                    let view = View::new(id, origin, size, window);
                    views.push(view);

                    // Update origin for next iteration of fold.
                    (views, origin + Size::rows(rows))
                });
            self.views = views;
            self.find_view(view_id)
        }
    }

    pub fn find_view(&self, id: u32) -> Option<&View> {
        self.views.iter().find(|view| view.id == id)
    }

    pub fn views(&self) -> Views {
        Views::new(self)
    }

    fn next_id(&mut self) -> u32 {
        let id = self.id_seq;
        self.id_seq += 1;
        id
    }
}
