//! A restricted environment available to editing functions.
//!
//! All editing functions, which are bound to key sequences at runtime, are external
//! to the core [`Editor`]. A restricted set of functions is necessary not only to
//! simplify operations, but more importantly, to enforce certain invariants.

use crate::clip::Clipboard;
use crate::editor::{Align, Editor, EditorRef, ImmutableEditor};
use crate::source::Source;
use crate::window::{BannerRef, WindowRef};
use crate::workspace::{Placement, WorkspaceRef};
use std::collections::{BTreeMap, HashMap};

/// Map of view ids to editor ids.
pub type ViewMap = HashMap<u32, u32>;

/// Map of editor ids to editors.
pub type EditorMap = BTreeMap<u32, EditorRef>;

pub struct Environment {
    pub workspace: WorkspaceRef,
    pub clipboard: Clipboard,
    editor_map: EditorMap,
    editor_id_seq: u32,
    view_map: ViewMap,
    active_view_id: u32,
}

pub enum Focus {
    Top,
    Bottom,
    Above,
    Below,
    To(u32),
}

impl Environment {
    /// Collection of predefined editors, all of which are _ephemeral_ and may not
    /// be removed from the list of editors.
    ///
    /// Note that this collection must not be empty, otherwise initialization of the
    /// environment will panic.
    const BUILTIN_EDITORS: [(u32, &'static str); 1] = [(0, "scratch")];

    pub fn new(workspace: WorkspaceRef) -> Environment {
        // Seed list of editors with builtins.
        let mut editor_map = EditorMap::new();
        for (id, name) in Self::BUILTIN_EDITORS {
            editor_map.insert(
                id,
                Editor::mutable(
                    workspace.borrow().config().clone(),
                    Source::Ephemeral(name.to_string()),
                    None,
                )
                .into_ref(),
            );
        }
        let editor_id_seq = editor_map.len() as u32;

        // Workspace guarantees that at least one view always exists, which can be
        // reliably fetched as top view. Attach this view to first builtin editor.
        let active_view_id = workspace.borrow().top_view().id;
        let editor_id = Self::BUILTIN_EDITORS[0].0;
        editor_map
            .get(&editor_id)
            .unwrap_or_else(|| panic!("expecting builtin editor id {editor_id}"))
            .borrow_mut()
            .attach(workspace.borrow().top_view().window.clone(), Align::Auto);
        let mut view_map = ViewMap::new();
        view_map.insert(active_view_id, editor_id);

        Environment {
            workspace,
            clipboard: Clipboard::new(),
            editor_map,
            editor_id_seq,
            view_map,
            active_view_id,
        }
    }

    /// Returns the id of the _active_ view.
    pub fn get_active_view_id(&self) -> u32 {
        self.active_view_id
    }

    /// Returns the id of the _active_ editor.
    pub fn get_active_editor_id(&self) -> u32 {
        self.get_view_editor_id_unchecked(self.active_view_id)
    }

    /// Returns a reference to the _active_ editor.
    pub fn get_active_editor(&self) -> &EditorRef {
        self.get_view_editor_unchecked(self.active_view_id)
    }

    /// Returns a reference to the editor attached to `view_id`.
    pub fn get_view_editor(&self, view_id: u32) -> &EditorRef {
        self.get_view_editor_unchecked(view_id)
    }

    /// Sets the _active_ view based on `focus` and returns the view id.
    pub fn set_active(&mut self, focus: Focus) -> u32 {
        self.unfocus(self.active_view_id);
        let ws = self.workspace.borrow();
        self.active_view_id = match focus {
            Focus::Top => ws.top_view().id,
            Focus::Bottom => ws.bottom_view().id,
            Focus::Above => ws.above_view(self.active_view_id).id,
            Focus::Below => ws.below_view(self.active_view_id).id,
            Focus::To(view_id) => {
                if self.view_map.contains_key(&view_id) {
                    view_id
                } else {
                    panic!("expecting view id {view_id}")
                }
            }
        };
        self.focus(self.active_view_id);
        self.active_view_id
    }

    /// Returns the view id attached to `editor_id` or `None` if unattached.
    pub fn find_editor_view_id(&self, editor_id: u32) -> Option<u32> {
        self.view_map
            .iter()
            .find(|(_, e_id)| **e_id == editor_id)
            .map(|(v_id, _)| *v_id)
    }

    /// Returns the id of the editor whose [`source`](Editor::source) matches `source`,
    /// otherwise `None`.
    pub fn find_editor_id(&self, source: &str) -> Option<u32> {
        self.editor_map
            .iter()
            .find(|(_, e)| e.borrow().source().to_string() == source)
            .map(|(id, _)| *id)
    }

    /// Opens a new window whose placement is specified by `place`, attaches `editor`
    /// to that window, and returns a tuple containing the new view id and editor id,
    /// or `None` if the workspace is unable to create the new view.
    pub fn open_editor(
        &mut self,
        editor: EditorRef,
        place: Placement,
        align: Align,
    ) -> Option<(u32, u32)> {
        let view_id = self.workspace.borrow_mut().open_view(place);
        view_id.map(|view_id| {
            let editor_id = self.add_editor(editor);
            self.reattach_views();
            self.attach_to_editor(view_id, editor_id, align);
            (view_id, editor_id)
        })
    }

    /// Opens a new window whose placement is specified by `place`, attaches the editor
    /// of `editor_id` to that window, and returns the new view id, or `None` if the
    /// workspace is unable to create the new view.
    ///
    /// If `editor_id` is already attached to an existing window, then a new window is
    /// not opened and the view id of the attached window is returned instead.
    pub fn open_window(&mut self, editor_id: u32, place: Placement, align: Align) -> Option<u32> {
        self.find_editor_view_id(editor_id).or_else(|| {
            let view_id = self.workspace.borrow_mut().open_view(place);
            view_id.inspect(|&view_id| {
                self.reattach_views();
                self.attach_to_editor(view_id, editor_id, align);
            })
        })
    }

    /// Attaches the window of `view_id` to `editor` and returns the new editor id.
    ///
    /// A side effect of this function is that the current editor, if any, associated
    /// with `view_id` is detached before attaching to the new editor.
    pub fn set_editor_for(&mut self, view_id: u32, editor: EditorRef, align: Align) -> u32 {
        let editor_id = self.add_editor(editor);
        self.attach_to_editor(view_id, editor_id, align);
        editor_id
    }

    pub fn set_editor(&mut self, editor: EditorRef, align: Align) -> u32 {
        self.set_editor_for(self.active_view_id, editor, align)
    }

    /// Attaches the window of `view_id` to the editor of `editor_id`, but only if
    /// `editor_id` is not already attached to another window.
    ///
    /// If the `editor_id` is already attached to another window, then the editor id
    /// attached to `view_id` is returned, otherwise `editor_id` is returned. In essence,
    /// a return value that differs from `editor_id` indicates that the intended switch
    /// did not happen.
    ///
    /// A side effect of this function is that the current editor, if any, associated
    /// with `view_id` is detached before attaching to the new editor.
    pub fn switch_editor_for(&mut self, view_id: u32, editor_id: u32, align: Align) -> u32 {
        let view_editor_id = self.get_view_editor_id_unchecked(view_id);
        if editor_id == view_editor_id {
            editor_id
        } else {
            self.view_map
                .values()
                .cloned()
                .find(|e_id| *e_id == editor_id)
                .map(|_| view_editor_id)
                .unwrap_or_else(|| {
                    self.attach_to_editor(view_id, editor_id, align);
                    editor_id
                })
        }
    }

    pub fn switch_editor(&mut self, editor_id: u32, align: Align) -> u32 {
        self.switch_editor_for(self.active_view_id, editor_id, align)
    }

    /// Closes the window of `view_id`, detaches the associated editor, and returns
    /// the id of the new _active_ view.
    ///
    /// This function returns `None` if the workspace is unable to close the window,
    /// which happens when it is the only remaining window.
    pub fn close_window_for(&mut self, view_id: u32) -> Option<u32> {
        let next_id = self.workspace.borrow_mut().close_view(view_id);
        next_id.map(|next_id| {
            self.remove_view(view_id);
            self.reattach_views();
            self.focus(next_id);
            self.active_view_id = next_id;
            self.active_view_id
        })
    }

    pub fn close_window(&mut self) -> Option<u32> {
        self.close_window_for(self.active_view_id)
    }

    /// Closes the window of `view_id`, detaches and possibly removes the associated
    /// editor, and returns the id of the new _active_ view.
    ///
    /// The associated editor is removed only if it is not a builtin.
    ///
    /// This function returns `None` if the workspace is unable to close the window,
    /// which happens when it is the only remaining window.
    pub fn kill_window_for(&mut self, view_id: u32) -> Option<u32> {
        let editor_id = self.get_view_editor_id_unchecked(view_id);
        let next_id = self.close_window_for(view_id);
        next_id.inspect(|_| {
            if !self.is_builtin(editor_id) {
                self.remove_editor_unchecked(editor_id);
            }
        })
    }

    pub fn kill_window(&mut self) -> Option<u32> {
        self.kill_window_for(self.active_view_id)
    }

    /// Closes the editor of `editor_id`, but only if the editor is not attached to a
    /// window and not a builtin, returning `editor_id` if closed and `None` otherwise.
    pub fn close_editor(&mut self, editor_id: u32) -> Option<u32> {
        if self.find_editor_view_id(editor_id).is_some() || self.is_builtin(editor_id) {
            None
        } else {
            self.remove_editor_unchecked(editor_id);
            Some(editor_id)
        }
    }

    /// Resizes the workspace, which might remove a subset of views if resizing
    /// violates the minimum size constraint for windows.
    pub fn resize(&mut self) {
        let view_ids = {
            let mut ws = self.workspace.borrow_mut();
            let ids = ws.resize(self.active_view_id);
            ws.clear_shared();
            ids
        };

        if let Some(view_ids) = view_ids {
            for view_id in view_ids {
                self.remove_view(view_id);
            }
            self.reattach_views();
            self.focus(self.active_view_id);
        }
    }

    pub fn editor_map(&self) -> &EditorMap {
        &self.editor_map
    }

    pub fn view_map(&self) -> &ViewMap {
        &self.view_map
    }

    /// Attaches the window of `view_id` to the editor referenced by `editor_id`, and
    /// also detaches the window from its current editor if an association exists.
    fn attach_to_editor(&mut self, view_id: u32, editor_id: u32, align: Align) {
        if let Some(id) = self.view_map.get(&view_id) {
            self.get_editor_unchecked(*id).borrow_mut().detach();
        }
        self.get_editor_unchecked(editor_id)
            .borrow_mut()
            .attach(self.window_of(view_id), align);
        self.view_map.insert(view_id, editor_id);
    }

    /// Rettach windows to editors for all views, which is necessary after the
    /// workspace adds or removes views, or resizes itself.
    fn reattach_views(&mut self) {
        for (view_id, editor_id) in self.view_map.iter() {
            self.get_editor_unchecked(*editor_id)
                .borrow_mut()
                .attach(self.window_of(*view_id), Align::Auto);
        }
    }

    /// Removes `view_id` and detaches the corresponding editor, returning the editor
    /// id that was detached.
    fn remove_view(&mut self, view_id: u32) -> u32 {
        let editor_id = self
            .view_map
            .remove(&view_id)
            .unwrap_or_else(|| panic!("expecting view id {view_id}"));
        self.get_editor_unchecked(editor_id).borrow_mut().detach();
        editor_id
    }

    fn add_editor(&mut self, editor: EditorRef) -> u32 {
        let editor_id = self.next_editor_id();
        self.editor_map.insert(editor_id, editor);
        editor_id
    }

    fn get_editor_unchecked(&self, editor_id: u32) -> &EditorRef {
        self.editor_map
            .get(&editor_id)
            .unwrap_or_else(|| panic!("expecting editor id {editor_id}"))
    }

    fn remove_editor_unchecked(&mut self, editor_id: u32) -> EditorRef {
        self.editor_map
            .remove(&editor_id)
            .unwrap_or_else(|| panic!("expecting editor id {editor_id}"))
    }

    fn get_view_editor_id_unchecked(&self, view_id: u32) -> u32 {
        *self
            .view_map
            .get(&view_id)
            .unwrap_or_else(|| panic!("expecting view id {view_id}"))
    }

    fn get_view_editor_unchecked(&self, view_id: u32) -> &EditorRef {
        self.get_editor_unchecked(self.get_view_editor_id_unchecked(view_id))
    }

    fn window_of(&self, view_id: u32) -> WindowRef {
        self.workspace.borrow().get_view(view_id).window.clone()
    }

    fn banner_of(&self, view_id: u32) -> BannerRef {
        self.window_of(view_id).borrow().banner().clone()
    }

    fn focus(&self, view_id: u32) {
        self.banner_of(view_id).borrow_mut().focus(true);
    }

    fn unfocus(&self, view_id: u32) {
        self.banner_of(view_id).borrow_mut().focus(false);
    }

    fn next_editor_id(&mut self) -> u32 {
        let id = self.editor_id_seq;
        self.editor_id_seq += 1;
        id
    }

    fn is_builtin(&self, editor_id: u32) -> bool {
        Self::BUILTIN_EDITORS.iter().any(|(id, _)| *id == editor_id)
    }
}
