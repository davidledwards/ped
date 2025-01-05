//! Sources of editor content.

use crate::sys;
use std::fmt::{self, Display, Formatter};
use std::time::SystemTime;

/// A representation of various editor sources.
#[derive(Clone)]
pub enum Source {
    /// A _null_ source indicating the absence of a source.
    Null,

    /// A _file_ source containing a path and an optional timestamp representing the
    /// last modification, or `None` if the file is new.
    File(String, Option<SystemTime>),

    /// An _ephemeral_ source with a name.
    Ephemeral(String),
}

impl Source {
    pub fn as_file(path: &str, timestamp: Option<SystemTime>) -> Source {
        Source::File(path.to_string(), timestamp)
    }

    pub fn as_ephemeral(name: &str) -> Source {
        Source::Ephemeral(name.to_string())
    }

    pub fn is_file(&self) -> bool {
        match self {
            Self::File(_, _) => true,
            _ => false,
        }
    }
}

impl Display for Source {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "?null"),
            Self::File(path, _) => write!(f, "{}", sys::pretty_path(path)),
            Self::Ephemeral(name) => write!(f, "@{name}"),
        }
    }
}
