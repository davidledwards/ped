//! Options parser.
use crate::error::{Error, Result};

pub struct Options {
    pub help: bool,
    pub version: bool,
    pub show_spotlight: bool,
    pub show_lines: bool,
    pub show_eol: bool,
    pub rc_path: Option<String>,
    pub files: Vec<String>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            help: false,
            version: false,
            files: vec![],
            show_spotlight: false,
            show_lines: false,
            show_eol: false,
            rc_path: None,
        }
    }
}

impl Options {
    pub fn parse<T>(args: T) -> Result<Options>
    where
        T: IntoIterator<Item = String>,
    {
        let mut opts = Options::default();
        let mut it = args.into_iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--help" => opts.help = true,
                "--version" => opts.version = true,
                "--show-spotlight" => opts.show_spotlight = true,
                "--show-lines" => opts.show_lines = true,
                "--show-eol" => opts.show_eol = true,
                "--rc" => opts.rc_path = Some(expect_value(&arg, it.next())?),
                arg if arg.starts_with("--") => return Err(Error::unexpected_arg(arg)),
                _ => opts.files.push(arg),
            }
        }
        Ok(opts)
    }
}

fn expect_value(arg: &str, next_arg: Option<String>) -> Result<String> {
    next_arg.ok_or_else(|| Error::expected_value(arg))
}
