//! Editing environment.
use crate::editor::{self, EditorRef};
use crate::window::WindowRef;
use crate::workspace::{Placement, Workspace, WorkspaceRef};
use std::cell::{Ref, RefMut};
use std::collections::{BTreeMap, HashMap};

/// Map of view ids to editor ids.
type ViewMap = HashMap<u32, u32>;

/// Map of editor ids to editors.
type EditorMap = BTreeMap<u32, EditorRef>;

pub struct Environment {
    workspace: WorkspaceRef,
    editor_map: EditorMap,
    editor_id_seq: u32,
    view_map: ViewMap,
    active_view_id: u32,
    clipboard: Option<Vec<char>>,
}

pub enum Focus {
    Top,
    Bottom,
    Above,
    Below,
}

impl Environment {
    /// Collection of predefined editors, all of which are *transient* and may not
    /// be removed from the list of editors.
    ///
    /// Note that this collection must not be empty, otherwise initialization of the
    /// environment will panic.
    const BUILTIN_EDITORS: [(u32, &'static str); 1] = [(0, "scratch")];

    pub fn new(workspace: WorkspaceRef) -> Environment {
        // Seed list of editors with builtins.
        let mut editor_map = EditorMap::new();
        for (id, name) in Self::BUILTIN_EDITORS {
            editor_map.insert(id, editor::transient(name, None));
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
            .attach(workspace.borrow().top_view().window.clone());
        let mut view_map = ViewMap::new();
        view_map.insert(active_view_id, editor_id);

        Environment {
            workspace,
            editor_map,
            editor_id_seq,
            view_map,
            active_view_id,
            clipboard: None,
        }
    }

    /// Returns the id of the *active* view.
    pub fn get_active(&self) -> u32 {
        self.active_view_id
    }

    /// Sets the *active* view based on `focus` and returns the view id.
    pub fn set_active(&mut self, focus: Focus) -> u32 {
        self.active_view_id = match focus {
            Focus::Top => self.workspace().top_view().id,
            Focus::Bottom => self.workspace().bottom_view().id,
            Focus::Above => self.workspace().above_view(self.active_view_id).id,
            Focus::Below => self.workspace().below_view(self.active_view_id).id,
        };
        self.active_view_id
    }

    /// Returns a reference to the editor associated with `view_id`.
    pub fn get_editor_for(&self, view_id: u32) -> &EditorRef {
        self.editor_of(view_id)
    }

    pub fn get_editor(&self) -> &EditorRef {
        self.get_editor_for(self.active_view_id)
    }

    /// Opens a new window whose placement is specified by `place`, attaches `editor`
    /// to that window, and returns the id of the new *active* view.
    ///
    /// This function returns `None` if the workspace is unable to create the new
    /// view.
    pub fn open_editor(&mut self, editor: EditorRef, place: Placement) -> Option<u32> {
        let view_id = self.workspace_mut().open_view(place);
        view_id.map(|view_id| {
            let editor_id = self.add_editor(editor);
            self.reattach_views();
            self.attach_to_editor(view_id, editor_id);
            self.active_view_id = view_id;
            self.active_view_id
        })
    }

    /// Attaches the window of `view_id` to `editor` and returns `view_id` as the
    /// new *active* view.
    ///
    /// A side effect of this function is that the current editor, if any, associated
    /// with `view_id` is detached before attaching to the new editor.
    pub fn set_editor_for(&mut self, view_id: u32, editor: EditorRef) -> u32 {
        let editor_id = self.add_editor(editor);
        self.attach_to_editor(view_id, editor_id);
        self.active_view_id = view_id;
        self.active_view_id
    }

    pub fn set_editor(&mut self, editor: EditorRef) -> u32 {
        self.set_editor_for(self.active_view_id, editor)
    }

    /// Closes the window of `view_id`, detaches the associated editor, and returns
    /// the id of the new *active* view.
    ///
    /// This function returns `None` if the workspace is unable to close the window,
    /// which happens when it is the only remaining window.
    pub fn close_window_for(&mut self, view_id: u32) -> Option<u32> {
        let next_id = self.workspace_mut().close_view(view_id);
        next_id.map(|next_id| {
            self.remove_view(view_id);
            self.reattach_views();
            self.active_view_id = next_id;
            self.active_view_id
        })
    }

    pub fn close_window(&mut self) -> Option<u32> {
        self.close_window_for(self.active_view_id)
    }

    /// Closes the window of `view_id`, detaches and possibly removes the associated
    /// editor, and returns the id of the new *active* view.
    ///
    /// The associated editor is removed only if it is not a builtin.
    ///
    /// This function returns `None` if the workspace is unable to close the window,
    /// which happens when it is the only remaining window.
    pub fn kill_window_for(&mut self, view_id: u32) -> Option<u32> {
        let editor_id = self.editor_id_of(view_id);
        let next_id = self.close_window_for(view_id);
        next_id.map(|next_id| {
            if !self.is_builtin(editor_id) {
                self.remove_editor_unchecked(editor_id);
            }
            next_id
        })
    }

    pub fn kill_window(&mut self) -> Option<u32> {
        self.kill_window_for(self.active_view_id)
    }

    pub fn set_clipboard(&mut self, text: Vec<char>) {
        self.clipboard = Some(text);
    }

    pub fn get_clipboard(&self) -> Option<&Vec<char>> {
        self.clipboard.as_ref()
    }

    pub fn resize(&mut self) {
        // Resize workspace, which might remove subset of views if resizing would
        // violate minimum size constraints.
        let view_ids = self.workspace_mut().resize(self.active_view_id);
        self.workspace_mut().clear_shared();

        if let Some(view_ids) = view_ids {
            for view_id in view_ids {
                self.remove_view(view_id);
            }
            self.reattach_views();
            self.get_editor().borrow_mut().show_cursor();
        }
    }

    /// Attaches the window of `view_id` to the editor referenced by `editor_id`, and
    /// also detaches the window from its current editor if an association exists.
    fn attach_to_editor(&mut self, view_id: u32, editor_id: u32) {
        if let Some(id) = self.view_map.get(&view_id) {
            self.get_editor_unchecked(*id).borrow_mut().detach();
        }
        self.get_editor_unchecked(editor_id)
            .borrow_mut()
            .attach(self.window_of(view_id));
        self.view_map.insert(view_id, editor_id);
    }

    /// Rettach windows to editors for all views, which is necessary after the
    /// workspace adds or removes views, or resizes itself.
    fn reattach_views(&mut self) {
        for (view_id, editor_id) in self.view_map.iter() {
            self.get_editor_unchecked(*editor_id)
                .borrow_mut()
                .attach(self.window_of(*view_id));
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

    fn editor_id_of(&self, view_id: u32) -> u32 {
        *self
            .view_map
            .get(&view_id)
            .unwrap_or_else(|| panic!("expecting view id {view_id}"))
    }

    fn editor_of(&self, view_id: u32) -> &EditorRef {
        self.get_editor_unchecked(self.editor_id_of(view_id))
    }

    fn window_of(&self, view_id: u32) -> WindowRef {
        self.workspace().get_view(view_id).window.clone()
    }

    fn next_editor_id(&mut self) -> u32 {
        let id = self.editor_id_seq;
        self.editor_id_seq += 1;
        id
    }

    fn is_builtin(&self, editor_id: u32) -> bool {
        Self::BUILTIN_EDITORS.iter().any(|(id, _)| *id == editor_id)
    }

    pub fn workspace(&self) -> Ref<'_, Workspace> {
        self.workspace.borrow()
    }

    fn workspace_mut(&self) -> RefMut<'_, Workspace> {
        self.workspace.borrow_mut()
    }
}
