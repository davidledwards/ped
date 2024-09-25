mod ansi;
mod bind;
mod buffer;
mod canvas;
mod color;
mod control;
mod display;
mod editor;
mod error;
mod io;
mod key;
mod op;
mod term;
mod window;
mod workspace;

use crate::bind::BindingMap;
use crate::buffer::Buffer;
use crate::control::Controller;
use crate::editor::Editor;
use crate::error::Result;
use crate::key::Keyboard;
use crate::workspace::Workspace;

use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Err(e) => {
            println!("{e:?}");
            ExitCode::from(1)
        }
        Ok(_) => ExitCode::SUCCESS,
    }
}

fn run() -> Result<()> {
    term::init()?;

    let bindings = BindingMap::new();

    let mut buffer = Buffer::new();
    let _ = io::read_file("TEST", &mut buffer)?;
    let pos = buffer.size() / 2;
    buffer.set_pos(pos);

    let (rows, cols) = term::size()?;
    let keyboard = Keyboard::new();
    let mut workspace = Workspace::new(rows, cols)?;
    let editor = Editor::new(buffer, workspace.new_window());

    let mut controller = Controller::new(keyboard, workspace, editor, bindings);
    controller.run()?;
    term::restore()?;
    Ok(())
}
