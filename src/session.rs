//! Editing session.
use crate::editor::{Editor, EditorRef};
use crate::window::Window;
use crate::workspace::{Placement, Workspace};

use std::cell::RefMut;
use std::collections::HashMap;

type EditorMap = HashMap<u32, EditorRef>;

pub struct Session {
    pub workspace: Workspace,
    pub editors: Vec<EditorRef>,
    pub editor_map: EditorMap,
    pub active_id: u32,
}

impl Session {
    pub fn new(workspace: Workspace, editors: Vec<EditorRef>) -> Session {
        let editors = if editors.len() == 0 {
            let editor = Editor::new();
            vec![editor.to_ref()]
        } else {
            editors
        };

        let mut workspace = workspace;
        let mut editor_map = EditorMap::new();
        for (i, editor) in editors.iter().enumerate() {
            let view = if i == 0 {
                workspace.top_view()
            } else {
                workspace
                    .add_view(Placement::Bottom)
                    .map(|id| workspace.get_view(id))
                    .unwrap_or_else(|| {
                        panic!("FIXME: adding view could fail because of space limits")
                    })
            };
            editor.borrow_mut().attach(view.window().clone());
            editor_map.insert(view.id(), editor.clone());
        }

        let active_id = workspace.top_view().id();

        Session {
            workspace,
            editors,
            editor_map,
            active_id,
        }
    }

    pub fn active_id(&self) -> u32 {
        self.active_id
    }

    pub fn active_editor(&self) -> RefMut<'_, Editor> {
        self.editor_map
            .get(&self.active_id)
            .unwrap_or_else(|| panic!("{}: active editor not found", self.active_id))
            .borrow_mut()
    }

    pub fn add_view(&mut self, place: Placement) -> Option<u32> {
        match self.workspace.add_view(place) {
            Some(id) => {
                for (id, e) in self.editor_map.iter() {
                    let view = self.workspace.get_view(*id);
                    e.borrow_mut().attach(view.window().clone());
                }

                let mut editor = Editor::new();
                editor.attach(self.workspace.get_view(id).window().clone());
                let editor = editor.to_ref();
                self.editors.push(editor.clone());

                self.editor_map.insert(id, editor);
                self.active_id = id;
                self.active_editor().show_cursor();
                Some(id)
            }
            None => {
                self.workspace.alert("out of window space");
                None
            }
        }
    }

    pub fn remove_view(&mut self, id: u32) -> Option<u32> {
        match self.workspace.remove_view(id) {
            Some(next_id) => {
                match self.editor_map.remove(&id) {
                    Some(e) => e.borrow_mut().attach(Window::zombie().to_ref()),
                    None => panic!("{}: view not found", id),
                }
                for (id, e) in self.editor_map.iter() {
                    let view = self.workspace.get_view(*id);
                    e.borrow_mut().attach(view.window().clone());
                }
                self.active_id = next_id;
                self.active_editor().show_cursor();
                Some(self.active_id)
            }
            None => {
                self.workspace.alert("cannot remove only window");
                None
            }
        }
    }

    pub fn prev_view(&mut self) -> u32 {
        self.active_id = self.workspace.above_view(self.active_id).id();
        self.active_editor().show_cursor();
        self.active_id
    }

    pub fn next_view(&mut self) -> u32 {
        self.active_id = self.workspace.below_view(self.active_id).id();
        self.active_editor().show_cursor();
        self.active_id
    }
}
