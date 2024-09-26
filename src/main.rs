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
mod opt;
mod term;
mod window;
mod workspace;

use crate::bind::BindingMap;
use crate::buffer::Buffer;
use crate::control::Controller;
use crate::editor::Editor;
use crate::error::Result;
use crate::key::Keyboard;
use crate::opt::Options;
use crate::workspace::Workspace;

use std::env;
use std::ops::Drop;
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

struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        if let Err(e) = term::restore() {
            println!("error resetting terminal");
        }
    }
}

fn run() -> Result<()> {
    let opts = Options::parse(env::args().skip(1))?;
    if opts.help {
        println!("usage: ped");
    } else {
        let bindings = BindingMap::new();

        let mut buffer = Buffer::new();
        if let Some(file) = opts.files.iter().next() {
            let _ = io::read_file(file, &mut buffer)?;
            buffer.set_pos(0);
        };

        let (rows, cols) = term::size()?;
        term::init()?;
        let _reset = Reset;

        let keyboard = Keyboard::new();
        let mut workspace = Workspace::new(rows, cols)?;
        let editor = Editor::new(buffer, workspace.new_window());
        let mut controller = Controller::new(keyboard, workspace, editor, bindings);
        controller.run()?
    }
    Ok(())
}
