//! Editing session.
use crate::editor::{Editor, EditorRef};
use crate::window::Window;
use crate::workspace::{Placement, Workspace};

use std::cell::RefMut;
use std::collections::HashMap;

type ViewMap = HashMap<u32, EditorRef>;

pub struct Session {
    pub workspace: Workspace,
    pub editors: Vec<EditorRef>,
    pub views: ViewMap,
    pub active: u32,
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
        let mut views = ViewMap::new();
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
            views.insert(view.id(), editor.clone());
        }

        let active = workspace.top_view().id();

        Session {
            workspace,
            editors,
            views,
            active,
        }
    }

    pub fn active_id(&self) -> u32 {
        self.active
    }

    pub fn active_editor(&self) -> RefMut<'_, Editor> {
        self.views
            .get(&self.active)
            .unwrap_or_else(|| panic!("{}: active editor not found", self.active))
            .borrow_mut()
    }

    pub fn add_view(&mut self, place: Placement) -> Option<u32> {
        match self.workspace.add_view(place) {
            Some(id) => {
                for (id, e) in self.views.iter() {
                    let view = self.workspace.get_view(*id);
                    e.borrow_mut().attach(view.window().clone());
                }

                let mut editor = Editor::new();
                editor.attach(self.workspace.get_view(id).window().clone());
                let editor = editor.to_ref();
                self.editors.push(editor.clone());

                self.views.insert(id, editor);
                self.active = id;
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
                match self.views.remove(&id) {
                    Some(e) => e.borrow_mut().attach(Window::zombie().to_ref()),
                    None => panic!("{}: view not found", id),
                }
                for (id, e) in self.views.iter() {
                    let view = self.workspace.get_view(*id);
                    e.borrow_mut().attach(view.window().clone());
                }
                self.active = next_id;
                Some(self.active)
            }
            None => {
                self.workspace.alert("cannot remove only window");
                None
            }
        }
    }
}
