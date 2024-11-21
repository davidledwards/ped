//! # ped
//!
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
mod complete;
mod config;
mod control;
mod echo;
mod editor;
mod env;
mod error;
mod grid;
mod input;
mod io;
mod key;
mod op;
mod opt;
mod size;
mod term;
mod window;
mod workspace;
mod writer;

use crate::bind::Bindings;
use crate::config::Configuration;
use crate::control::Controller;
use crate::error::Result;
use crate::key::Keyboard;
use crate::opt::Options;
use crate::workspace::Workspace;
use std::ops::Drop;
use std::process::ExitCode;

/// Usage documentation for display to terminal.
const USAGE: &str = include_str!("usage.in");

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
    } else {
        run_opts(&opts)
    }
}

fn run_opts(opts: &Options) -> Result<()> {
    let mut config = if let Some(ref config_path) = opts.config_path {
        Configuration::load_file(config_path)?
    } else {
        Configuration::load()?
    };
    config.apply_opts(opts);
    run_config(opts, config)
}

fn run_config(opts: &Options, config: Configuration) -> Result<()> {
    let keyboard = Keyboard::new();
    let bindings = Bindings::new(&config.bindings);
    let workspace = Workspace::new(config);
    let mut controller = Controller::new(keyboard, bindings, workspace);
    controller.open(&opts.files)?;

    term::init()?;
    let _restore = RestoreTerminal;
    controller.run()
}
