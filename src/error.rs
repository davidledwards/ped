//! Error types.

use std::fmt::{self, Display, Formatter};
use std::io;
use std::str::Utf8Error;

/// A convenient `Result` type whose error type is [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// The set of possible errors.
#[derive(Debug)]
pub enum Error {
    OS(io::Error),
    IO(io::Error),
    UTF8(Utf8Error),
    UnexpectedArg(String),
    BindKey(String),
    BindOp(String),
    File(String, io::Error),
}

impl Error {
    pub fn os() -> Error {
        Error::OS(io::Error::last_os_error())
    }

    pub fn os_cloning(e: &io::Error) -> Error {
        Error::OS(io::Error::new(e.kind(), e.to_string()))
    }

    pub fn unexpected_arg(arg: &str) -> Error {
        Error::UnexpectedArg(arg.to_string())
    }

    pub fn bind_key(key: &str) -> Error {
        Error::BindKey(key.to_string())
    }

    pub fn bind_op(op: &str) -> Error {
        Error::BindOp(op.to_string())
    }

    pub fn file(path: &str, e: io::Error) -> Error {
        Error::File(path.to_string(), e)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::OS(e) => write!(f, "OS error: {e}"),
            Error::IO(e) => write!(f, "I/O error: {e}"),
            Error::UTF8(e) => write!(f, "UTF-8 error: {e}"),
            Error::UnexpectedArg(arg) => write!(f, "{arg}: unexpected argument"),
            Error::BindKey(key) => write!(f, "{key}: unknown key for binding"),
            Error::BindOp(op) => write!(f, "{op}: unknown operation for binding"),
            Error::File(path, e) => write!(f, "{path}: {e}"),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Error {
        Error::UTF8(e)
    }
}
