//! A simple parser for CLI options.

use crate::error::{Error, Result};
use std::str::FromStr;

/// Represents all potential CLI options.
pub struct Options {
    pub help: bool,
    pub version: bool,
    pub spotlight: Option<bool>,
    pub lines: Option<bool>,
    pub eol: Option<bool>,
    pub tab_hard: Option<bool>,
    pub tab_size: Option<u32>,
    pub keys: bool,
    pub ops: bool,
    pub bindings: bool,
    pub colors: bool,
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
            colors: false,
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
                "--help" | "-h" => opts.help = true,
                "--version" | "-v" => opts.version = true,
                "--spotlight" => opts.spotlight = Some(true),
                "--no-spotlight" => opts.spotlight = Some(false),
                "--lines" => opts.lines = Some(true),
                "--no-lines" => opts.lines = Some(false),
                "--eol" => opts.eol = Some(true),
                "--no-eol" => opts.eol = Some(false),
                "--tab-hard" => opts.tab_hard = Some(true),
                "--tab-soft" => opts.tab_hard = Some(false),
                "--tab-size" | "-t" => opts.tab_size = Some(parse_arg(&arg, it.next())?),
                "--keys" => opts.keys = true,
                "--ops" => opts.ops = true,
                "--bindings" => opts.bindings = true,
                "--colors" => opts.colors = true,
                "--config" | "-C" => opts.config_path = Some(expect_value(&arg, it.next())?),
                "--syntax" | "-S" => opts.syntax_dir = Some(expect_value(&arg, it.next())?),
                "--bare" | "-b" => opts.bare = true,
                "--bare-syntax" | "-B" => opts.bare_syntax = true,
                "--" => {
                    // All arguments following `--` are interpreted as files.
                    opts.files.extend(it);
                    break;
                }
                arg if arg.starts_with("--") || arg.starts_with("-") => {
                    return Err(Error::unexpected_arg(arg))
                }
                _ => {
                    // Any other match is presumed to be a file.
                    opts.files.push(arg)
                }
            }
        }
        Ok(opts)
    }
}

/// Parses `next_arg`, which is presumed to be the value that follows `arg`.
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

/// Verifies that `next_arg` is present, which is presumed to be the value that
/// follows `arg`.
fn expect_value(arg: &str, next_arg: Option<String>) -> Result<String> {
    next_arg.ok_or_else(|| Error::expected_value(arg))
}
