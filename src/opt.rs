//! Options parser.
use crate::error::{Error, Result};

pub struct Options {
    pub help: bool,
    pub version: bool,
    pub files: Vec<String>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            help: false,
            version: false,
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
                arg if arg.starts_with("--") => return Err(Error::unexpected_arg(arg)),
                _ => opts.files.push(arg),
            }
        }
        Ok(opts)
    }
}
