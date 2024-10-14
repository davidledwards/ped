mod ansi;
mod bind;
mod buffer;
mod canvas;
mod color;
mod control;
mod display;
mod editor;
mod env;
mod error;
mod grid;
mod input;
mod io;
mod key;
mod op;
mod opt;
mod term;
mod theme;
mod window;
mod workspace;

use display::Point;
use input::LineEditor;
use key::Key;

use crate::bind::Bindings;
use crate::buffer::Buffer;
use crate::control::Controller;
use crate::editor::Editor;
use crate::error::Result;
use crate::key::Keyboard;
use crate::opt::Options;
use crate::theme::Theme;
use crate::workspace::Workspace;

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
    let opts = Options::parse(std::env::args().skip(1))?;
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

        let mut keyboard = Keyboard::new();
        let theme = Theme::new();
        let mut input = LineEditor::new(Point::new(0, 0), 30, theme.to_ref(), "open file:");
        loop {
            let key = keyboard.read()?;
            if key == Key::Control(17) {
                break;
            } else {
                input.process_key(&key);
            }
        }
        // let workspace = Workspace::new(theme);
        // let mut controller = Controller::new(keyboard, bindings, workspace, editors);
        // controller.run()?
    }
    Ok(())
}
