//! The *ped*estrian text editor.
//!
//! Copyright 2024 David Edwards
//!
//! Licensed under the Apache License, Version 2.0 (the "License");
//! you may not use this file except in compliance with the License.
//! You may obtain a copy of the License at
//!
//! <https://www.apache.org/licenses/LICENSE-2.0>
//!
//! Unless required by applicable law or agreed to in writing, software
//! distributed under the License is distributed on an "AS IS" BASIS,
//! WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//! See the License for the specific language governing permissions and
//! limitations under the License.
mod ansi;
mod bind;
mod buffer;
mod canvas;
mod color;
mod config;
mod control;
mod echo;
mod editor;
mod env;
mod error;
mod etc;
mod grid;
mod help;
mod input;
mod io;
mod key;
mod op;
mod opt;
mod search;
mod size;
mod syntax;
mod sys;
mod term;
mod token;
mod user;
mod window;
mod workspace;
mod writer;

use crate::config::Configuration;
use crate::control::Controller;
use crate::error::Result;
use crate::key::Keyboard;
use crate::opt::Options;
use crate::syntax::Registry;
use crate::workspace::Workspace;
use std::ops::Drop;
use std::process::ExitCode;

/// Usage documentation for display to terminal.
const USAGE: &str = include_str!("include/usage.in");

// Version and build information.
const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_HASH: &str = env!("BUILD_HASH");
const BUILD_DATE: &str = env!("BUILD_DATE");

/// Used for restoring the terminal via [`Drop`] to its original state.
struct RestoreTerminal;

impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        term::restore().unwrap_or_else(|e| println!("error restoring terminal: {e}"));
    }
}

fn main() -> ExitCode {
    match run() {
        Err(e) => {
            println!("{e}");
            ExitCode::from(1)
        }
        Ok(_) => ExitCode::SUCCESS,
    }
}

fn run() -> Result<()> {
    let opts = Options::parse(std::env::args().skip(1))?;
    if opts.help {
        println!("{USAGE}");
        Ok(())
    } else if opts.version {
        println!("{PACKAGE_NAME} {PACKAGE_VERSION} ({BUILD_HASH} {BUILD_DATE})");
        Ok(())
    } else if opts.keys {
        print!("{}", help::keys_content());
        Ok(())
    } else if opts.ops {
        print!("{}", help::ops_content());
        Ok(())
    } else {
        run_opts(&opts)
    }
}

fn run_opts(opts: &Options) -> Result<()> {
    // Load optional configuration from either standard location or path specified on
    // command line, and apply command line options afterwards since these override
    // all other settings.
    let mut config = if let Some(ref config_path) = opts.config_path {
        Configuration::load_file(config_path)?
    } else {
        Configuration::load()?
    };
    config.apply_opts(opts);

    // Load optional syntax configurations via registry and update configuration.
    config.registry = if let Some(ref syntax_dir) = opts.syntax_dir {
        Registry::load_dir(syntax_dir, &config.colors)?
    } else {
        Registry::load(&config.colors)?
    };

    if opts.bindings {
        print!("{}", help::bindings_content(config.bindings.bindings()));
        Ok(())
    } else {
        prepare_term();
        run_config(opts, config)?;
        restore_term();
        Ok(())
    }
}

fn run_config(opts: &Options, config: Configuration) -> Result<()> {
    // Initialize main controller and open files specified on command line.
    let mut controller = Controller::new(Keyboard::new(), Workspace::new(config));
    controller.open(&opts.files)?;

    // Puts terminal into raw mode prior to running main controller loop, but also
    // ensures terminal settings are restored upon exit.
    term::init()?;
    {
        let _restore = RestoreTerminal;
        controller.run();
    }
    Ok(())
}

fn prepare_term() {
    print!(
        "{}{}{}",
        ansi::alt_screen(true),
        ansi::track_mouse(true),
        ansi::clear_screen()
    );
}

fn restore_term() {
    print!(
        "{}{}{}",
        ansi::clear_screen(),
        ansi::track_mouse(false),
        ansi::alt_screen(false)
    );
}
