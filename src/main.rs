mod ansi;
mod bind;
mod buffer;
mod canvas;
mod color;
mod control;
mod display;
mod editor;
mod error;
mod grid;
mod io;
mod key;
mod op;
mod opt;
mod session;
mod term;
mod theme;
mod window;
mod workspace;

use crate::bind::Bindings;
use crate::buffer::Buffer;
use crate::control::Controller;
use crate::editor::Editor;
use crate::error::Result;
use crate::key::Keyboard;
use crate::opt::Options;
use crate::theme::Theme;
use crate::workspace::Workspace;

use std::env;
use std::ops::Drop;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Err(e) => {
            println!("{e}");
            ExitCode::from(1)
        }
        Ok(_) => ExitCode::SUCCESS,
    }
}

struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        if let Err(e) = term::restore() {
            println!("error resetting terminal: {e:?}");
        }
    }
}

fn run() -> Result<()> {
    let opts = Options::parse(env::args().skip(1))?;
    if opts.help {
        println!("usage: ped");
    } else {
        let editors = if let Some(file) = opts.files.iter().next() {
            let mut buffer = Buffer::new();
            let _ = io::read_file(file, &mut buffer)?;
            buffer.set_pos(0);
            vec![Editor::with_buffer(Some(PathBuf::from(file)), buffer.to_ref()).to_ref()]
        } else {
            Vec::new()
        };

        let bindings = Bindings::new();

        term::init()?;
        let _reset = Reset;

        let keyboard = Keyboard::new();
        let theme = Theme::new();
        let workspace = Workspace::new(theme);
        let mut controller = Controller::new(keyboard, bindings, workspace, editors);
        controller.run()?
    }
    Ok(())
}
