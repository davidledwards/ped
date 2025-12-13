//! Echo messages to the shared region of a workspace.

use crate::color::Color;
use crate::workspace::WorkspaceRef;
use crate::writer::Writer;

pub struct Echo {
    workspace: WorkspaceRef,
    echo_color: Color,
    text: Option<String>,
}

impl Echo {
    pub fn new(workspace: WorkspaceRef) -> Echo {
        let config = workspace.borrow().config.clone();
        let echo_color = Color::new(config.theme.echo_fg, config.theme.text_bg);

        Echo {
            workspace,
            echo_color,
            text: None,
        }
    }

    pub fn set(&mut self, text: String) {
        self.text = Some(text);
        self.draw();
    }

    pub fn clear(&mut self) {
        self.text = None;
        self.draw();
    }

    pub fn draw(&mut self) {
        if let Some(text) = &self.text {
            let (origin, size) = self.workspace.borrow().shared_region();

            // Possibly clip text to fit size constraint of viewable region.
            let chars = text.chars().take(size.cols as usize).collect::<Vec<_>>();
            let blank_cols = size.cols - chars.len() as u32;

            Writer::new_at(origin)
                .set_color(self.echo_color)
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
