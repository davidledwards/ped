//! A complete collection of errors.

use std::error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::str;
use toml::de;

/// A convenient `Result` type whose error type is [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// The set of possible errors.
#[derive(Debug)]
pub enum Error {
    /// An I/O error reported by the operating system.
    Os { cause: io::Error },

    /// An I/O error resulting from an operation on a file referenced by `path`.
    Io { path: String, cause: io::Error },

    /// A UTF-8 encoding/decoding error with the offending sequence of `bytes`.
    Utf8 {
        bytes: Vec<u8>,
        cause: str::Utf8Error,
    },

    /// An unexpected command-line argument `arg`.
    UnexpectedArg { arg: String },

    /// A value is expected for a command-line argument `arg`.
    ExpectedValue { arg: String },

    /// A `value` given for a command-line argument `arg` is not valid.
    InvalidValue { arg: String, value: String },

    /// A `key` name given in a key binding is not valid.
    InvalidKey { key: String },

    /// An operation `op` given in a key binding is not valid.
    InvalidOp { op: String },

    /// An error occurred while parsing a configuration file referenced by `path`.
    Configuration { path: String, cause: String },
}

impl error::Error for Error {}

impl Error {
    pub fn os() -> Error {
        Error::Os {
            cause: io::Error::last_os_error(),
        }
    }

    pub fn os_cloning(e: &io::Error) -> Error {
        Error::Os {
            cause: io::Error::new(e.kind(), e.to_string()),
        }
    }

    pub fn io(path: &str, cause: io::Error) -> Error {
        Error::Io {
            path: path.to_string(),
            cause,
        }
    }

    pub fn utf8(bytes: &[u8], cause: str::Utf8Error) -> Error {
        Error::Utf8 {
            bytes: bytes.to_vec(),
            cause,
        }
    }

    pub fn unexpected_arg(arg: &str) -> Error {
        Error::UnexpectedArg {
            arg: arg.to_string(),
        }
    }

    pub fn expected_value(arg: &str) -> Error {
        Error::ExpectedValue {
            arg: arg.to_string(),
        }
    }

    pub fn invalid_value(arg: &str, value: &str) -> Error {
        Error::InvalidValue {
            arg: arg.to_string(),
            value: value.to_string(),
        }
    }

    pub fn invalid_key(key: &str) -> Error {
        Error::InvalidKey {
            key: key.to_string(),
        }
    }

    pub fn invalid_op(op: &str) -> Error {
        Error::InvalidOp { op: op.to_string() }
    }

    pub fn configuration(path: &str, e: &de::Error) -> Error {
        Error::Configuration {
            path: path.to_string(),
            cause: format!("{e}"),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Os { cause } => write!(f, "I/O error: {cause}"),
            Error::Io { path, cause } => write!(f, "{path}: {cause}"),
            Error::Utf8 { bytes, cause } => write!(f, "{bytes:?}: {cause}"),
            Error::UnexpectedArg { arg } => write!(f, "{arg}: unexpected argument"),
            Error::ExpectedValue { arg } => write!(f, "{arg}: expecting value to follow"),
            Error::InvalidValue { arg, value } => {
                write!(f, "{value}: invalid value following {arg}")
            }
            Error::InvalidKey { key } => write!(f, "{key}: invalid key"),
            Error::InvalidOp { op } => write!(f, "{op}: invalid operation"),
            Error::Configuration { path, cause } => {
                write!(f, "{path}: configuration error: {cause}")
            }
        }
    }
}
