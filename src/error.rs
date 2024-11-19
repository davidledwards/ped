//! Error types.
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
    OS {
        cause: io::Error,
    },
    IO {
        device: Option<String>,
        cause: io::Error,
    },
    UTF8 {
        bytes: Vec<u8>,
        cause: str::Utf8Error,
    },
    UnexpectedArg {
        arg: String,
    },
    ExpectedValue {
        arg: String,
    },
    InvalidKey {
        key: String,
    },
    BindOp {
        op: String,
    },
    Configuration {
        path: String,
        cause: String,
    },
}

impl error::Error for Error {}

impl Error {
    pub fn os() -> Error {
        Error::OS {
            cause: io::Error::last_os_error(),
        }
    }

    pub fn os_cloning(e: &io::Error) -> Error {
        Error::OS {
            cause: io::Error::new(e.kind(), e.to_string()),
        }
    }

    pub fn io(device: Option<&str>, cause: io::Error) -> Error {
        Error::IO {
            device: device.map(|d| d.to_string()),
            cause,
        }
    }

    pub fn utf8(bytes: &[u8], cause: str::Utf8Error) -> Error {
        Error::UTF8 {
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

    pub fn invalid_key(key: &str) -> Error {
        Error::InvalidKey {
            key: key.to_string(),
        }
    }

    pub fn bind_op(op: &str) -> Error {
        Error::BindOp { op: op.to_string() }
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
            Error::OS { cause } => write!(f, "OS error: {cause}"),
            Error::IO { device, cause } => match device {
                Some(d) => write!(f, "{d}: {cause}"),
                None => write!(f, "{cause}"),
            },
            Error::UTF8 { bytes, cause } => write!(f, "{bytes:?}: {cause}"),
            Error::UnexpectedArg { arg } => write!(f, "{arg}: unexpected argument"),
            Error::ExpectedValue { arg } => write!(f, "{arg}: expecting value to follow"),
            Error::InvalidKey { key } => write!(f, "{key}: invalid key"),
            Error::BindOp { op } => write!(f, "{op}: bind operation not found"),
            Error::Configuration { path, cause } => {
                write!(f, "{path}: configuration error: {cause}")
            }
        }
    }
}
