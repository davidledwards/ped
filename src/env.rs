//! Editing environment.
use crate::editor::{self, Editor, EditorRef};
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
    view_id: u32,
    clipboard: Option<Vec<char>>,
}

impl Environment {
    /// Collection of predefined editors, all of which are *transient* and may not
    /// be removed from the list of editors.
    ///
    /// Note that this collection must not be empty, otherwise initialization of the
    /// environment will panic.
    const BUILTIN_EDITORS: [(u32, &'static str); 2] = [(0, "scratch"), (1, "log")];

    pub fn new(workspace: WorkspaceRef) -> Environment {
        // Seed list of editors with builtins.
        let mut editor_map = EditorMap::new();
        for (id, name) in Self::BUILTIN_EDITORS {
            editor_map.insert(id, editor::transient(name, None));
        }
        let editor_id_seq = editor_map.len() as u32;

        // Workspace guarantees that at least single view always exists, which can
        // be reliably fetched as top view.
        let view_id = workspace.borrow().top_view().id;
        let mut this = Environment {
            workspace,
            editor_map,
            editor_id_seq,
            view_map: ViewMap::new(),
            view_id,
            clipboard: None,
        };

        // Attach first builtin editor to initial view in workspace.
        let editor_id = Self::BUILTIN_EDITORS[0].0;
        this.get_editor(editor_id)
            .borrow_mut()
            .attach(this.window_of(this.view_id));
        this.view_map.insert(view_id, editor_id);
        this
    }

    /// Returns the *id* of the active view.
    pub fn active_view(&self) -> u32 {
        self.view_id
    }

    /// Returns a mutable reference to the editor associated with the active view.
    pub fn active_editor(&self) -> RefMut<'_, Editor> {
        self.get_view_editor(self.view_id).borrow_mut()
    }

    fn attach_view(&mut self, view_id: u32, editor: EditorRef) -> u32 {
        editor.borrow_mut().attach(self.window_of(view_id));
        let editor_id = self.next_editor_id();
        self.editor_map.insert(editor_id, editor);
        self.view_map.insert(view_id, editor_id);
        editor_id
    }

    pub fn resize(&mut self) {
        // Resize workspace, which might remove subset of views if resizing would
        // violate minimum size constraints.
        let view_ids = self.workspace_mut().resize(self.view_id);
        self.workspace_mut().clear_shared();

        if let Some(view_ids) = view_ids {
            for view_id in view_ids {
                self.remove_view(view_id);
            }
            self.reattach_views();
            self.active_editor().show_cursor();
        }
    }

    pub fn set_view(&mut self, editor: EditorRef) -> u32 {
        self.active_editor().detach();
        self.attach_view(self.view_id, editor);
        self.active_editor().show_cursor();
        self.view_id
    }

    pub fn open_view(&mut self, editor: EditorRef, place: Placement) -> Option<u32> {
        let view_id = self.workspace_mut().open_view(place);
        view_id.map(|view_id| {
            self.reattach_views();
            self.attach_view(view_id, editor);
            self.view_id = view_id;
            self.active_editor().show_cursor();
            self.view_id
        })
    }

    pub fn close_view(&mut self, view_id: u32) -> Option<u32> {
        let next_id = self.workspace_mut().close_view(view_id);
        next_id.map(|next_id| {
            self.remove_view(view_id);
            self.reattach_views();
            self.view_id = next_id;
            self.active_editor().show_cursor();
            self.view_id
        })
    }

    pub fn top_view(&mut self) -> u32 {
        let top_id = self.workspace().top_view().id;
        self.view_id = top_id;
        self.active_editor().show_cursor();
        self.view_id
    }

    pub fn bottom_view(&mut self) -> u32 {
        let bottom_id = self.workspace().bottom_view().id;
        self.view_id = bottom_id;
        self.active_editor().show_cursor();
        self.view_id
    }

    pub fn prev_view(&mut self) -> u32 {
        let prev_id = self.workspace().above_view(self.view_id).id;
        self.view_id = prev_id;
        self.active_editor().show_cursor();
        self.view_id
    }

    pub fn next_view(&mut self) -> u32 {
        let next_id = self.workspace().below_view(self.view_id).id;
        self.view_id = next_id;
        self.active_editor().show_cursor();
        self.view_id
    }

    pub fn set_clipboard(&mut self, text: Vec<char>) {
        self.clipboard = Some(text);
    }

    pub fn get_clipboard(&self) -> Option<&Vec<char>> {
        self.clipboard.as_ref()
    }

    /// Rettach windows to editors for all views, which is necessary after the
    /// workspace adds or removes views, or resizes itself.
    fn reattach_views(&mut self) {
        for (view_id, editor_id) in self.view_map.iter() {
            self.get_editor(*editor_id)
                .borrow_mut()
                .attach(self.window_of(*view_id));
        }
    }

    /// Removes `view_id` and detaches the corresponding editor, returning the
    /// editor id that was detached.
    fn remove_view(&mut self, view_id: u32) -> u32 {
        let editor_id = self
            .view_map
            .remove(&view_id)
            .unwrap_or_else(|| panic!("expecting view id {view_id}"));
        self.get_editor(editor_id).borrow_mut().detach();
        editor_id
    }

    fn get_editor(&self, editor_id: u32) -> &EditorRef {
        self.editor_map
            .get(&editor_id)
            .unwrap_or_else(|| panic!("expecting editor id {editor_id}"))
    }

    fn get_view(&self, view_id: u32) -> u32 {
        *self
            .view_map
            .get(&view_id)
            .unwrap_or_else(|| panic!("expecting view id {view_id}"))
    }

    fn get_view_editor(&self, view_id: u32) -> &EditorRef {
        self.get_editor(self.get_view(view_id))
    }

    fn window_of(&self, view_id: u32) -> WindowRef {
        self.workspace().get_view(view_id).window.clone()
    }

    fn next_editor_id(&mut self) -> u32 {
        let id = self.editor_id_seq;
        self.editor_id_seq += 1;
        id
    }

    pub fn workspace(&self) -> Ref<'_, Workspace> {
        self.workspace.borrow()
    }

    fn workspace_mut(&self) -> RefMut<'_, Workspace> {
        self.workspace.borrow_mut()
    }
}
