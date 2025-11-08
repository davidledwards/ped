//! A simple parser for CLI options.

use crate::error::{Error, Result};
use std::str::FromStr;

/// Represents all potential CLI options.
pub struct Options {
    pub spotlight: Option<bool>,
    pub lines: Option<bool>,
    pub eol: Option<bool>,
    pub tab_hard: Option<bool>,
    pub tab_size: Option<u32>,
    pub crlf: Option<bool>,
    pub keys: bool,
    pub ops: bool,
    pub bindings: bool,
    pub colors: bool,
    pub theme: bool,
    pub describe: Option<String>,
    pub track_lateral: Option<bool>,
    pub config_path: Option<String>,
    pub syntax_dir: Option<String>,
    pub bare: bool,
    pub bare_syntax: bool,
    pub help: bool,
    pub version: bool,
    pub source: bool,
    pub files: Vec<String>,
}

#[allow(clippy::derivable_impls, reason = "retain expressiveness")]
impl Default for Options {
    fn default() -> Options {
        Options {
            spotlight: None,
            lines: None,
            eol: None,
            tab_hard: None,
            tab_size: None,
            crlf: None,
            keys: false,
            ops: false,
            bindings: false,
            colors: false,
            theme: false,
            describe: None,
            track_lateral: None,
            config_path: None,
            syntax_dir: None,
            bare: false,
            bare_syntax: false,
            help: false,
            version: false,
            source: false,
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
                "--spotlight" => opts.spotlight = Some(true),
                "--no-spotlight" => opts.spotlight = Some(false),
                "--lines" => opts.lines = Some(true),
                "--no-lines" => opts.lines = Some(false),
                "--eol" => opts.eol = Some(true),
                "--no-eol" => opts.eol = Some(false),
                "--tab-hard" => opts.tab_hard = Some(true),
                "--tab-soft" => opts.tab_hard = Some(false),
                "--tab-size" | "-t" => opts.tab_size = Some(parse_arg(&arg, it.next())?),
                "--crlf" => opts.crlf = Some(true),
                "--lf" => opts.crlf = Some(false),
                "--keys" => opts.keys = true,
                "--ops" => opts.ops = true,
                "--bindings" => opts.bindings = true,
                "--colors" => opts.colors = true,
                "--theme" => opts.theme = true,
                "--describe" => opts.describe = Some(expect_value(&arg, it.next())?),
                "--track-lateral" => opts.track_lateral = Some(true),
                "--no-track-lateral" => opts.track_lateral = Some(false),
                "--config" | "-C" => opts.config_path = Some(expect_value(&arg, it.next())?),
                "--syntax" | "-S" => opts.syntax_dir = Some(expect_value(&arg, it.next())?),
                "--bare" | "-b" => opts.bare = true,
                "--bare-syntax" | "-B" => opts.bare_syntax = true,
                "--help" | "-h" | "-?" => opts.help = true,
                "--version" | "-v" => opts.version = true,
                "--source" => opts.source = true,
                "--" => {
                    // All arguments following `--` are interpreted as files.
                    opts.files.extend(it);
                    break;
                }
                arg if arg.starts_with("--") || arg.starts_with("-") => {
                    return Err(Error::unexpected_arg(arg));
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
            .map_err(|_| Error::invalid_value(arg, &value))
    } else {
        Err(Error::expected_value(arg))
    }
}

/// Verifies that `next_arg` is present, which is presumed to be the value that
/// follows `arg`.
fn expect_value(arg: &str, next_arg: Option<String>) -> Result<String> {
    next_arg.ok_or_else(|| Error::expected_value(arg))
}
