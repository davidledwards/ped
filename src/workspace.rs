//! Workspace management.
use crate::display::{Display, Point, Size};
use crate::term;
use crate::theme::{Theme, ThemeRef};
use crate::window::{Window, WindowRef};

use std::cell::RefCell;
use std::cmp;
use std::rc::Rc;

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
/// Mutiple views within a workspace are organized vertically with an equal number of
/// rows. As views are added and removed, the resulting collection of views is resized
/// accorndingly.
///
/// A workspace always provides at least `1` view, which implies that the last
/// remaining view can never be removed.
pub struct Workspace {
    size: Size,
    theme: ThemeRef,
    views_origin: Point,
    views_size: Size,
    shared_origin: Point,
    shared_size: Size,
    id_seq: u32,
    views: Vec<View>,
}

pub type WorkspaceRef = Rc<RefCell<Workspace>>;

impl Workspace {
    /// A lower bound on the size of the workspace area.
    const MIN_SIZE: Size = Size::new(3, 2);

    /// An adjustment to subtract from the workspace area size for calculating the area
    /// size of views.
    const VIEWS_SIZE_ADJUST: Size = Size::rows(1);

    /// Minimum number of rows assigned to a view.
    const MIN_VIEW_ROWS: u32 = 2;

    /// Creates a workspace with the given `theme` and consuming the entire terminal.
    pub fn new(theme: Theme) -> Workspace {
        let size = Self::query_size();
        let mut this = Workspace {
            size,
            theme: theme.to_ref(),
            views_origin: Point::ORIGIN,
            views_size: size - Self::VIEWS_SIZE_ADJUST,
            shared_origin: Point::ORIGIN + Size::rows(size.rows - 1),
            shared_size: Size::new(1, size.cols),
            id_seq: 0,
            views: vec![],
        };
        this.open_view(Placement::Top);
        this
    }

    /// Turns the workspace into a [`WorkspaceRef`].
    pub fn to_ref(self) -> WorkspaceRef {
        Rc::new(RefCell::new(self))
    }

    pub fn shared_region(&self) -> (Point, Size) {
        (self.shared_origin, self.shared_size)
    }

    pub fn theme(&self) -> &ThemeRef {
        &self.theme
    }

    /// Opens a new view in the workspace whose placement is based on `place`, returning
    /// the *id* of the view or `None` if the view could not be created.
    ///
    /// Existing views will be resized as a side effect of opening a new view. However,
    /// the view will not be created, and resizing will not occur, if the resulting
    /// number of rows would drop below [`Self::MIN_ROWS`].
    ///
    /// This function panics if the `id` specified in [`Placement::Above`] or
    /// [`Placement::Below`] is not found, as this would indicate a correctness
    /// problem by the caller.
    pub fn open_view(&mut self, place: Placement) -> Option<u32> {
        // Calculate number of rows that would need to be allocated to each view
        // should another view be added.
        let rows = self.views_size.rows / (self.views.len() + 1) as u32;
        if rows < Self::MIN_VIEW_ROWS {
            None
        } else {
            // Find correct index for insertion of new window.
            let index = match place {
                Placement::Top => 0,
                Placement::Bottom => self.views.len(),
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

            // Insert zombie view in correct place before resizing views.
            let view_id = self.next_id();
            self.views.insert(index, self.create_zombie(view_id));
            self.resize_views();
            Some(view_id)
        }
    }

    /// Closes the view referenced by `id` from the workspace, returning the *id* of
    /// the view above or `None` if the view could not be closed.
    ///
    /// Remaining views will be resized as a side effect of removal. However, the view
    /// will not be closed, and resizing will not occur, if `id` is the only remaining
    /// view in the workspace.
    ///
    /// This function panics if `id` is not found, as this would indicate a correctness
    /// problem by the caller.
    pub fn close_view(&mut self, id: u32) -> Option<u32> {
        if self.views.len() > 1 {
            let i = self
                .views
                .iter()
                .position(|v| v.id == id)
                .unwrap_or_else(|| panic!("{id}: view not found"));
            self.views.remove(i);
            self.resize_views();

            // Select view above the one removed.
            let i = if i > 0 { i - 1 } else { 0 };
            Some(self.views[i].id)
        } else {
            None
        }
    }

    /// Resizes the workspace if the terminal size has changed and returns a vector of
    /// view *ids* removed due to minimum size constraints of the workspace.
    ///
    /// Under most circumstances, the vector is empty. However, if one or more views
    /// needs to be removed, the selection starts at the bottom and proceeds up.
    ///
    /// There exists the possibility that all but one view are removed if the terminal
    /// is made small enough. Since the workspace guarantees the existence of at least
    /// one view, `keep_id` is provided by the caller to identify which view should be
    /// kept.
    pub fn resize(&mut self, keep_id: u32) -> Option<Vec<u32>> {
        let size = Self::query_size();
        if size != self.size {
            // Update size of workspace and view areas, which drive calculation of total
            // number of views and corresponding row allocations.
            self.size = size;
            self.views_size = size - Self::VIEWS_SIZE_ADJUST;
            self.shared_origin = Point::ORIGIN + Size::rows(size.rows - 1);
            self.shared_size = Size::new(1, size.cols);

            // Calculate number of rows to allocate to each view, though revised workspace
            // size might lead to violation of minimum view size constraint, which means
            // total number of views must be reduced such that constraint is held.
            let rows = self.views_size.rows / self.views.len() as u32;
            let count = if rows < Self::MIN_VIEW_ROWS {
                (self.views_size.rows / Self::MIN_VIEW_ROWS) as usize
            } else {
                self.views.len()
            };

            // If necessary, remove views from bottom to top, though do not remove view
            // specified by caller regardless of where it exists in stack of views.
            let removed_ids =
                if count < self.views.len() {
                    let n = self.views.len() - count;
                    let indexes = self.views.iter().rev().enumerate().fold(
                        Vec::new(),
                        |mut indexes, (i, v)| {
                            if indexes.len() < n && v.id() != keep_id {
                                // Index is flipped since views are being iterated back to
                                // front.
                                indexes.push(self.views.len() - i - 1);
                            }
                            indexes
                        },
                    );
                    indexes.iter().map(|i| self.views.remove(*i).id()).collect()
                } else {
                    vec![]
                };

            self.resize_views();
            Some(removed_ids)
        } else {
            None
        }
    }

    /// Resizes views with an equal distribution of `rows`, though views towards the top
    /// will include an additional row if `residual_rows` is greater than 0.
    fn resize_views(&mut self) {
        let count = self.views.len();
        let rows = self.views_size.rows / count as u32;
        let residual_rows = self.views_size.rows % count as u32;

        let (views, _) = self.views.iter().enumerate().fold(
            (Vec::new(), self.views_origin),
            |(mut views, origin), (i, v)| {
                // Give precedence of residual rows to top-most views.
                let rows = if i >= residual_rows as usize {
                    rows
                } else {
                    rows + 1
                };

                // Recreate view with new origin and size.
                let view = self.create_view(v.id, origin, rows);
                views.push(view);

                // Update origin for next iteration of fold.
                (views, origin + Size::rows(rows))
            },
        );
        self.views = views;
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

    pub fn clear_shared(&mut self) {
        Display::new(self.shared_origin)
            .set_cursor(Point::ORIGIN)
            .set_color(self.theme.echo_color)
            .write_str(" ".repeat(self.size.cols as usize).as_str())
            .send();
    }

    /// Returns the terminal [size](Size), but possibly changes what is reported by the
    /// terminal to ensure the lower bound constraint of [`MIN_SIZE`](Self::MIN_SIZE)
    /// holds true.
    fn query_size() -> Size {
        let (rows, cols) =
            term::size().unwrap_or_else(|e| panic!("trying to query terminal size: {e}"));
        Size::new(
            cmp::max(rows, Self::MIN_SIZE.rows),
            cmp::max(cols, Self::MIN_SIZE.cols),
        )
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

    fn create_zombie(&self, id: u32) -> View {
        View::new(id, Point::ORIGIN, Size::ZERO, Window::zombie().to_ref())
    }
}
