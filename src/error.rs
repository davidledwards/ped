//! Error handling.

use std::io;

/// A convenient `Result` type whose error type is [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// The set of possible errors.
#[derive(Debug)]
pub enum Error {
    /// An out of memory error.
    OutOfMemory,

    /// An attempt was made to allocate a buffer that is too large.
    BufferTooLarge(usize),

    /// An I/O error.
    IO(io::Error),

    /// A UTF-8 decoding error.
    UTF8(std::str::Utf8Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Error {
        Error::UTF8(e)
    }
}
