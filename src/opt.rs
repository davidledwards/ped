//! Options parser.

use std::str::FromStr;

use crate::error::{Error, Result};

pub struct Options {
    pub help: bool,
    pub version: bool,
    pub spotlight: Option<bool>,
    pub lines: Option<bool>,
    pub eol: Option<bool>,
    pub tab_hard: Option<bool>,
    pub tab_size: Option<usize>,
    pub keys: bool,
    pub ops: bool,
    pub bindings: bool,
    pub config_path: Option<String>,
    pub syntax_dir: Option<String>,
    pub bare: bool,
    pub bare_syntax: bool,
    pub files: Vec<String>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            help: false,
            version: false,
            spotlight: None,
            lines: None,
            eol: None,
            tab_hard: None,
            tab_size: None,
            keys: false,
            ops: false,
            bindings: false,
            config_path: None,
            syntax_dir: None,
            bare: false,
            bare_syntax: false,
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
                "--spotlight" => opts.spotlight = Some(true),
                "--lines" => opts.lines = Some(true),
                "--eol" => opts.eol = Some(true),
                "--tab-hard" => opts.tab_hard = Some(true),
                "--tab-soft" => opts.tab_hard = Some(false),
                "--tab-size" => opts.tab_size = Some(parse_arg(&arg, it.next())?),
                "--keys" => opts.keys = true,
                "--ops" => opts.ops = true,
                "--bindings" => opts.bindings = true,
                "--config" => opts.config_path = Some(expect_value(&arg, it.next())?),
                "--syntax" => opts.syntax_dir = Some(expect_value(&arg, it.next())?),
                "--bare" => opts.bare = true,
                "--bare-syntax" => opts.bare_syntax = true,
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
