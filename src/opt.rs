//! Options parser.
use crate::error::{Error, Result};

pub struct Options {
    pub help: bool,
    pub version: bool,
    pub show_spotlight: Option<bool>,
    pub show_lines: Option<bool>,
    pub show_eol: Option<bool>,
    pub rc_path: Option<String>,
    pub files: Vec<String>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            help: false,
            version: false,
            show_spotlight: None,
            show_lines: None,
            show_eol: None,
            rc_path: None,
            files: vec![],
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
                "--show-spotlight" => opts.show_spotlight = Some(true),
                "--show-lines" => opts.show_lines = Some(true),
                "--show-eol" => opts.show_eol = Some(true),
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
