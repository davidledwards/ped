//! Options parser.
use std::str::FromStr;

use crate::error::{Error, Result};

pub struct Options {
    pub help: bool,
    pub version: bool,
    pub show_spotlight: Option<bool>,
    pub show_lines: Option<bool>,
    pub show_eol: Option<bool>,
    pub tab_size: Option<usize>,
    pub config_path: Option<String>,
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
            tab_size: None,
            config_path: None,
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
                "--tab-size" => opts.tab_size = Some(parse_arg(&arg, it.next())?),
                "--config" => opts.config_path = Some(expect_value(&arg, it.next())?),
                arg if arg.starts_with("--") => return Err(Error::unexpected_arg(arg)),
                _ => opts.files.push(arg),
            }
        }
        Ok(opts)
    }
}

fn parse_arg<T>(arg: &str, next_arg: Option<String>) -> Result<T>
where
    T: FromStr,
{
    if let Some(value) = next_arg {
        value
            .parse::<T>()
            .or_else(|_| Err(Error::invalid_value(arg, &value)))
    } else {
        Err(Error::expected_value(arg))
    }
}

fn expect_value(arg: &str, next_arg: Option<String>) -> Result<String> {
    next_arg.ok_or_else(|| Error::expected_value(arg))
}
