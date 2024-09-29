//! Main controller.
use crate::bind::BindingMap;
use crate::buffer::Buffer;
use crate::editor::{Editor, EditorRef};
use crate::error::Result;
use crate::key::{Key, Keyboard};
use crate::workspace::{Placement, Workspace};

pub struct Controller {
    keyboard: Keyboard,
    bindings: BindingMap,
    workspace: Workspace,
    editors: Vec<EditorRef>,
    editing: EditorRef,
}

const CTRL_X: u8 = 24;

impl Controller {
    pub fn new(
        keyboard: Keyboard,
        bindings: BindingMap,
        workspace: Workspace,
        editors: Vec<EditorRef>,
    ) -> Controller {
        let editors = if editors.len() == 0 {
            let editor = Editor::new(Buffer::new().to_ref());
            vec![editor.to_ref()]
        } else {
            editors
        };

        let mut workspace = workspace;
        for (i, editor) in editors.iter().enumerate() {
            let view = if i == 0 {
                workspace.default_view()
            } else {
                workspace.add_view(Placement::Bottom).unwrap_or_else(|| {
                    panic!("FIXME: adding view could fail because of space limits")
                })
            };
            editor.borrow_mut().attach(view.window().clone());
        }

        let editing = editors[0].clone();

        Controller {
            keyboard,
            bindings,
            workspace,
            editors,
            editing,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            let key = self.keyboard.read()?;
            match self.bindings.lookup(&key) {
                Some(binding) => binding(&mut self.editing.borrow_mut(), &key)?,
                None => match key {
                    Key::None => {
                        // check for change in terminal size and update workspace
                    }
                    Key::Control(CTRL_X) => break,
                    _ => {}
                },
            }
        }
        Ok(())
    }
}
