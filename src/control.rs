//! Main controller.
use crate::bind::BindingMap;
use crate::editor::{Editor, EditorRef};
use crate::error::Result;
use crate::key::{Key, Keyboard};
use crate::op::{Action, ContinueFn};
use crate::window::Window;
use crate::workspace::{Placement, Workspace};

use std::cell::RefMut;
use std::collections::HashMap;

type ViewMap = HashMap<u32, EditorRef>;

pub struct Controller {
    keyboard: Keyboard,
    bindings: BindingMap,
    workspace: Workspace,
    editors: Vec<EditorRef>,
    views: ViewMap,
    editing: (u32, EditorRef),
}

impl Controller {
    pub fn editor(&self) -> RefMut<'_, Editor> {
        let (_, editor) = &self.editing;
        editor.borrow_mut()
    }

    pub fn workspace(&mut self) -> &mut Workspace {
        &mut self.workspace
    }

    pub fn new(
        keyboard: Keyboard,
        bindings: BindingMap,
        workspace: Workspace,
        editors: Vec<EditorRef>,
    ) -> Controller {
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

        let editing = (workspace.top_view().id(), editors[0].clone());

        Controller {
            keyboard,
            bindings,
            workspace,
            editors,
            views,
            editing,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut continue_fn: Option<ContinueFn> = None;
        loop {
            let key = self.keyboard.read()?;
            match key {
                Key::None => {
                    // do background stuff here
                }
                _ => {
                    let action = if let Some(mut cont_fn) = continue_fn.take() {
                        cont_fn(self, &key)?
                    } else {
                        match self.bindings.lookup(&key) {
                            Some(op_fn) => op_fn(self, &key)?,
                            None => {
                                match key {
                                    Key::Function(1) => {
                                        // HACK: add window to bottom
                                        match self.workspace.add_view(Placement::Bottom) {
                                            Some(id) => {
                                                // need to reattach windows to existing editors
                                                for (id, e) in self.views.iter() {
                                                    let view = self.workspace.get_view(*id);
                                                    e.borrow_mut().attach(view.window().clone());
                                                }

                                                // create new editor and attach window from new view
                                                let mut editor = Editor::new();
                                                editor.attach(
                                                    self.workspace.get_view(id).window().clone(),
                                                );
                                                let editor = editor.to_ref();
                                                self.editors.push(editor.clone());

                                                // add view/editor map of views
                                                self.views.insert(id, editor.clone());

                                                // make this new editor the area of focus
                                                self.editing = (id, editor.clone());
                                            }
                                            None => {
                                                self.workspace.alert("no space for new window");
                                            }
                                        }
                                    }
                                    Key::Function(2) => {
                                        // HACK: remove current window
                                        let (id, _) = self.editing;
                                        match self.workspace.remove_view(id) {
                                            Some(_) => {
                                                // remove from views
                                                match self.views.remove(&id) {
                                                    Some(e) => e
                                                        .borrow_mut()
                                                        .attach(Window::zombie().to_ref()),
                                                    None => panic!("{id}: should exist in views"),
                                                }

                                                // need to reattach windows
                                                for (id, e) in self.views.iter() {
                                                    let view = self.workspace.get_view(*id);
                                                    e.borrow_mut().attach(view.window().clone());
                                                }

                                                // need to pick editor for focus
                                                // FIXME: unwrap?
                                                let id = self.workspace.bottom_view().id();
                                                let editor =
                                                    self.views.get(&id).unwrap_or_else(|| {
                                                        panic!("{id}: should exist in views")
                                                    });
                                                self.editing = (id, editor.clone());
                                            }
                                            None => {
                                                self.workspace.alert("cannot remove only window");
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                                Action::Nothing
                            }
                        }
                    };
                    match action {
                        Action::Nothing => (),
                        Action::Quit => break,
                        Action::Continue(cont_fn) => continue_fn = Some(cont_fn),
                    }
                }
            }
        }
        Ok(())
    }
}
