//! Main controller.
use crate::bind::BindingMap;
use crate::editor::EditorRef;
use crate::error::Result;
use crate::key::{Key, Keyboard};
use crate::op::{Action, ContinueFn};
use crate::session::Session;
use crate::workspace::Workspace;

use std::collections::HashMap;

pub type ViewMap = HashMap<u32, EditorRef>;

pub struct Controller {
    keyboard: Keyboard,
    bindings: BindingMap,
    session: Session,
}

impl Controller {
    pub fn new(
        keyboard: Keyboard,
        bindings: BindingMap,
        workspace: Workspace,
        editors: Vec<EditorRef>,
    ) -> Controller {
        let session = Session::new(workspace, editors);

        Controller {
            keyboard,
            bindings,
            session,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut continue_fn: Option<Box<ContinueFn>> = None;
        loop {
            let key = self.keyboard.read()?;
            match key {
                Key::None => {
                    // do background stuff here
                }
                _ => {
                    let action = if let Some(mut cont_fn) = continue_fn.take() {
                        cont_fn(&mut self.session, &key)?
                    } else {
                        match self.bindings.lookup(&key) {
                            Some(op_fn) => op_fn(&mut self.session, &key)?,
                            None => Action::Nothing,
                        }
                    };
                    match action {
                        Action::Nothing => (),
                        Action::Continue(cont_fn) => continue_fn = Some(cont_fn),
                        Action::Alert(text) => self.session.workspace.alert(text.as_str()),
                        Action::Quit => break,
                    }
                }
            }
        }
        Ok(())
    }
}
