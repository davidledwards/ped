//! Echo messages to the shared region of a workspace.

use crate::workspace::WorkspaceRef;
use crate::writer::Writer;

pub struct Echo {
    workspace: WorkspaceRef,
    text: Option<String>,
}

impl Echo {
    pub fn new(workspace: WorkspaceRef) -> Echo {
        Echo {
            workspace,
            text: None,
        }
    }

    pub fn set(&mut self, text: &str) {
        self.text = Some(text.to_string());
        self.draw();
    }

    pub fn clear(&mut self) {
        self.text = None;
        self.draw();
    }

    pub fn draw(&mut self) {
        if let Some(ref text) = self.text {
            let (origin, size) = self.workspace.borrow().shared_region();

            // Possibly clip text to fit size constraint of viewable region.
            let chars = text.chars().take(size.cols as usize).collect::<Vec<_>>();
            let blank_cols = size.cols - chars.len() as u32;

            Writer::new_at(origin)
                .set_color(self.workspace.borrow().config().colors.echo)
                .write_str(chars.into_iter().collect::<String>().as_str())
                .write_str(" ".repeat(blank_cols as usize).as_str())
                .send();
        } else {
            self.workspace.borrow_mut().clear_shared();
        }
    }

    pub fn resize(&mut self) {
        self.draw();
    }
}
