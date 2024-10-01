//! Workspace management.
use crate::display::{Display, Point, Size};
use crate::theme::{Theme, ThemeRef};
use crate::window::{Window, WindowRef};

/// Placement directive when adding new [`View`]s to a [`Workspace`].
#[derive(Debug)]
pub enum Placement {
    /// Place at the top of the workspace.
    Top,

    /// Place at the bottom of the workspace.
    Bottom,

    /// Place directly above the view referenced by the contained *id*.
    Above(u32),

    /// Place directly below the view referenced by the contained *id*.
    Below(u32),
}

/// A view inside a [`Workspace`].
pub struct View {
    id: u32,
    origin: Point,
    size: Size,
    window: WindowRef,
}

impl View {
    fn new(id: u32, origin: Point, size: Size, window: WindowRef) -> View {
        View {
            id,
            origin,
            size,
            window,
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

/// A workspace is a collection of [`View`]s that encapsulate the entire editing
/// experience.
///
/// Usually, a workspace occupies the entire terminal, but this is not required, hence
/// the reason for providing [origin](`Point`) and [size](`Size`) informatiom.
///
/// Mutiple views within a workspace are organized vertically with an equal number of
/// rows. As views are added and removed, the resulting collection of views is resized
/// accorndingly.
///
/// A workspace always provides at least `1` view, which implies that the last
/// remaining view can never be removed.
pub struct Workspace {
    origin: Point,
    size: Size,
    theme: ThemeRef,
    views_origin: Point,
    views_size: Size,
    id_seq: u32,
    views: Vec<View>,
}

impl Workspace {
    /// An offset from the workspace origin for calculating the origin of views.
    const VIEWS_ORIGIN_OFFSET: Size = Size::ZERO;

    /// An adjustment of the workspace size for calculating the size of views.
    const VIEWS_SIZE_ADJUST: Size = Size::rows(1);

    /// Minimum number of rows assigned to a view.
    const MIN_ROWS: u32 = 3;

    pub fn new(origin: Point, size: Size, theme: Theme) -> Workspace {
        let mut this = Workspace {
            origin,
            size,
            theme: theme.to_ref(),
            views_origin: origin + Self::VIEWS_ORIGIN_OFFSET,
            views_size: size - Self::VIEWS_SIZE_ADJUST,
            id_seq: 0,
            views: vec![],
        };
        this.add_view(Placement::Top);
        this
    }

    /// Adds a view to the workspace whose placement is based on `place`, returning
    /// the *id* of the view or `None` if the view could not be created.
    ///
    /// Existing views will be resized as a side effect of adding a new view. However,
    /// the view will not be created, and resizing will not occur, if the resulting
    /// number of rows would drop below [`Self::MIN_ROWS`].
    ///
    /// This function panics if the `id` specified in [`Placement::Above`] or
    /// [`Placement::Below`] is not found, as this would indicate a correctness
    /// problem by the caller.
    pub fn add_view(&mut self, place: Placement) -> Option<u32> {
        // Determine number of rows to allocate to each view.
        let view_count = self.views.len() + 1;
        let view_rows = self.views_size.rows / view_count as u32;
        let residual_rows = self.views_size.rows % view_count as u32;

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
                    .unwrap_or_else(|| panic!("{place:?}: view not found")),
                Placement::Below(id) => self
                    .views
                    .iter()
                    .position(|v| v.id == id)
                    .map(|i| i + 1)
                    .unwrap_or_else(|| panic!("{place:?}: view not found")),
            };

            // Resize views.
            let (views, _) =
                (0..view_count).fold((Vec::new(), self.views_origin), |(mut views, origin), i| {
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

                    // Recreate view with new origin and size.
                    let view = self.create_view(id, origin, rows);
                    views.push(view);

                    // Update origin for next iteration of fold.
                    (views, origin + Size::rows(rows))
                });
            self.views = views;
            Some(view_id)
        }
    }

    /// Removes the view referenced by `id` from the workspace, returning the *id* of
    /// the following view or `None` if the view could not be removed.
    ///
    /// Remaining views will be resized as a side effect of removal. However, the view
    /// will not be removed, and resizing will not occur, if `id` is the only remaining
    /// view in the workspace.
    ///
    /// This function panics if `id` is not found, as this would indicate a correctness
    /// problem by the caller.
    pub fn remove_view(&mut self, id: u32) -> Option<u32> {
        if self.views.len() > 1 {
            let i = self
                .views
                .iter()
                .position(|v| v.id == id)
                .unwrap_or_else(|| panic!("{id}: view not found"));
            self.views.remove(i);

            // Determine number of rows to allocate to each view.
            let view_count = self.views.len();
            let view_rows = self.views_size.rows / view_count as u32;
            let residual_rows = self.views_size.rows % view_count as u32;

            // Resize views.
            let (views, _) = self.views.iter().enumerate().fold(
                (Vec::new(), self.views_origin),
                |(mut views, origin), (i, v)| {
                    // Give preference of residual rows to top-most windows.
                    let rows = if i >= residual_rows as usize {
                        view_rows
                    } else {
                        view_rows + 1
                    };

                    // Recreate view with new origin and size.
                    let view = self.create_view(v.id, origin, rows);
                    views.push(view);

                    // Update origin for next iteration of fold.
                    (views, origin + Size::rows(rows))
                },
            );
            self.views = views;
            let next_id = self.views[if i < self.views.len() { i } else { 0 }].id;
            Some(next_id)
        } else {
            None
        }
    }

    /// Returns the top-most [`View`] in the workspace.
    pub fn top_view(&self) -> &View {
        self.views
            .first()
            .unwrap_or_else(|| panic!("at least one view must always exist"))
    }

    /// Returns the bottom-most [`View`] in the workspace.
    pub fn bottom_view(&self) -> &View {
        self.views
            .last()
            .unwrap_or_else(|| panic!("at least one view must always exist"))
    }

    /// Returns the [`View`] above `id`, which might be itself if only one view exists.
    pub fn above_view(&self, id: u32) -> &View {
        let i = self
            .views
            .iter()
            .position(|v| v.id == id)
            .unwrap_or_else(|| panic!("{id}: view not found"));

        let n = self.views.len();
        let i = if i == 0 { n - 1 } else { i - 1 };
        &self.views[i]
    }

    /// Returns the [`View`] below `id`, which might be itself if only one view exists.
    pub fn below_view(&self, id: u32) -> &View {
        let i = self
            .views
            .iter()
            .position(|v| v.id == id)
            .unwrap_or_else(|| panic!("{id}: view not found"));

        let n = self.views.len();
        let i = (i + 1) % n;
        &self.views[i]
    }

    /// Returns the view corresponding to `id`, which must exist.
    ///
    /// This function panics if `id` is not found.
    pub fn get_view(&self, id: u32) -> &View {
        self.find_view(id)
            .unwrap_or_else(|| panic!("{id}: view not found"))
    }

    /// Returns the view associated with `id`, otherwise `None`.
    pub fn find_view(&self, id: u32) -> Option<&View> {
        self.views.iter().find(|v| v.id == id)
    }

    /// Returns an iterator over the [`View`]s.
    pub fn views(&self) -> Views {
        Views::new(self)
    }

    pub fn alert(&mut self, text: &str) {
        let text = if text.len() > self.size.cols as usize {
            &text[..self.size.cols as usize]
        } else {
            text
        };
        Display::new(self.origin + Size::rows(self.size.rows - 1))
            .set_cursor(Point::ORIGIN)
            .set_color(self.theme.alert_color)
            .write_str(text)
            .write_str(" ".repeat(self.size.cols as usize - text.len()).as_str())
            .send();
    }

    fn next_id(&mut self) -> u32 {
        let id = self.id_seq;
        self.id_seq += 1;
        id
    }

    fn create_view(&self, id: u32, origin: Point, rows: u32) -> View {
        let size = Size::new(rows, self.views_size.cols);
        let window = Window::new(origin, size, self.theme.clone());
        View::new(id, origin, size, window.to_ref())
    }
}
