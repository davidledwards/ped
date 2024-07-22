mod ansi;
mod buffer;
mod canvas;
mod color;
mod control;
mod display;
mod editor;
mod error;
mod io;
mod key;
mod keymap;
mod term;
mod window;
mod workspace;

use buffer::Buffer;
use control::Controller;
use editor::Editor;
use error::Error;
use key::Keyboard;
use workspace::Workspace;

fn main() -> Result<(), Error> {
    term::init()?;

    let mut buffer = Buffer::new()?;
    let _ = io::read_file("TEST", &mut buffer)?;
    let pos = buffer.size() / 2;
    buffer.set_pos(pos);

    let keyboard = Keyboard::new();
    let mut workspace = Workspace::new()?;
    let editor = Editor::new(buffer, workspace.new_window());

    let mut controller = Controller::new(keyboard, workspace, editor);
    controller.run()?;
    term::restore()?;
    Ok(())
}
