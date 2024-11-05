//! Editing environment.
use crate::editor::{Editor, EditorRef};
use crate::window::Window;
use crate::workspace::{Placement, Workspace, WorkspaceRef};
use std::cell::{Ref, RefMut};
use std::collections::HashMap;

/// Map of [`View`](crate::workspace::View) id to [`Editor`].
type EditorMap = HashMap<u32, EditorRef>;

pub struct Environment {
    workspace: WorkspaceRef,
    editors: Vec<EditorRef>,
    editor_map: EditorMap,
    active_id: u32,
}

impl Environment {
    /// Create an environment using `workspace` and the collection of `editors`.
    ///
    /// A view is created for each editor. However, if the number of editors exceeds the
    /// capacity of the workspace, then it stops creating views. If no editors are given,
    /// then an empty editor will be created and attached to the default view in the
    /// workspace.
    pub fn new(workspace: WorkspaceRef, editors: Vec<EditorRef>) -> Environment {
        // Need at least one editor.
        let editors = if editors.len() == 0 {
            vec![Editor::new().to_ref()]
        } else {
            editors
        };

        // Try creating views for each editor, stopping when workspace can no longer
        // add views because of capacity constraints.
        let mut editor_map = EditorMap::new();
        let workspace = workspace.clone();
        for (i, editor) in editors.iter().enumerate() {
            let mut ws = workspace.borrow_mut();
            let view = if i == 0 {
                Some(ws.top_view())
            } else {
                ws.open_view(Placement::Bottom).map(|id| ws.get_view(id))
            };
            if let Some(v) = view {
                editor.borrow_mut().attach(v.window().clone());
                editor_map.insert(v.id(), editor.clone());
            } else {
                break;
            }
        }

        // Active view is always top of workspace.
        let active_id = workspace.borrow().top_view().id();
        Environment {
            workspace,
            editors,
            editor_map,
            active_id,
        }
    }

    /// Returns the *id* of the active view.
    pub fn active_id(&self) -> u32 {
        self.active_id
    }

    /// Returns a mutable reference to the editor associated with the active view.
    pub fn active_editor(&self) -> RefMut<'_, Editor> {
        self.editor_map
            .get(&self.active_id)
            .unwrap_or_else(|| panic!("{}: active editor not found", self.active_id))
            .borrow_mut()
    }

    pub fn resize(&mut self) {
        let ids = self.workspace_mut().resize(self.active_id);
        if let Some(ids) = ids {
            for id in ids {
                if let Some(e) = self.editor_map.remove(&id) {
                    e.borrow_mut().attach(Window::zombie().to_ref());
                } else {
                    panic!("{id}: view not found");
                }
            }
            for (id, e) in self.editor_map.iter() {
                e.borrow_mut()
                    .attach(self.workspace().get_view(*id).window().clone());
            }
            self.active_editor().show_cursor();
        }
    }

    pub fn open_view(&mut self, place: Placement) -> Option<u32> {
        let id = self.workspace_mut().open_view(place);
        id.map(|id| {
            for (id, e) in self.editor_map.iter() {
                e.borrow_mut()
                    .attach(self.workspace().get_view(*id).window().clone());
            }

            let mut editor = Editor::new();
            editor.attach(self.workspace().get_view(id).window().clone());
            let editor = editor.to_ref();
            self.editors.push(editor.clone());

            self.editor_map.insert(id, editor);
            self.active_id = id;
            self.active_editor().show_cursor();
            id
        })
    }

    pub fn close_view(&mut self, id: u32) -> Option<u32> {
        let next_id = self.workspace_mut().close_view(id);
        next_id.map(|next_id| {
            match self.editor_map.remove(&id) {
                Some(e) => e.borrow_mut().attach(Window::zombie().to_ref()),
                None => panic!("{}: view not found", id),
            }
            for (id, e) in self.editor_map.iter() {
                let ws = self.workspace();
                let view = ws.get_view(*id);
                e.borrow_mut().attach(view.window().clone());
            }
            self.active_id = next_id;
            self.active_editor().show_cursor();
            self.active_id
        })
    }

    pub fn prev_view(&mut self) -> u32 {
        let prev_id = self.workspace().above_view(self.active_id).id();
        self.active_id = prev_id;
        self.active_editor().show_cursor();
        self.active_id
    }

    pub fn next_view(&mut self) -> u32 {
        let next_id = self.workspace().below_view(self.active_id).id();
        self.active_id = next_id;
        self.active_editor().show_cursor();
        self.active_id
    }

    pub fn get_editor(&self, id: u32) -> Option<EditorRef> {
        self.editor_map.get(&id).cloned()
    }

    pub fn workspace(&self) -> Ref<'_, Workspace> {
        self.workspace.borrow()
    }

    fn workspace_mut(&self) -> RefMut<'_, Workspace> {
        self.workspace.borrow_mut()
    }
}
