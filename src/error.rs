//! Error types.
use std::error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::str;

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
    BindKey {
        key: String,
    },
    BindOp {
        op: String,
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

    pub fn bind_key(key: &str) -> Error {
        Error::BindKey {
            key: key.to_string(),
        }
    }

    pub fn bind_op(op: &str) -> Error {
        Error::BindOp { op: op.to_string() }
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
            Error::BindKey { key } => write!(f, "{key}: unknown key for binding"),
            Error::BindOp { op } => write!(f, "{op}: unknown operation for binding"),
        }
    }
}
