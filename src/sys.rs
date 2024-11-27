//! # System functions
//!
//! A collection of functions that make common system-level operations easier to
//! perform. In most cases, these are convenience wrappers that reduce verbosity
//! and take an opinionated stance on how to interpret errors.
use std::env;
use std::path::{Path, PathBuf};

pub trait AsString {
    fn as_string(&self) -> String;
}

impl AsString for PathBuf {
    fn as_string(&self) -> String {
        self.display().to_string()
    }
}

impl AsString for Path {
    fn as_string(&self) -> String {
        self.display().to_string()
    }
}

/// Returns the `HOME` path as derived from the environment, or [`this_dir`] if an
/// error occurred while getting the value of `HOME`.
pub fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(|path| PathBuf::from(path))
        .unwrap_or_else(|| this_dir())
}

/// Returns the path of the working directory, or [`this_dir`] if an error occurred
/// while retrieving the value.
pub fn working_dir() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| this_dir())
}

/// Returns the path for `"."`.
pub fn this_dir() -> PathBuf {
    PathBuf::from(".")
}

/// Returns the base directory of `path`.
///
/// If `path` is either empty or a single component value, such as `"foo"`, the base
/// directory will be [`this_dir`]. If `path` terminates in a root, then the path
/// itself is returned.
pub fn base_dir<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    path.parent()
        .map(|parent| {
            if parent == Path::new("") {
                this_dir()
            } else {
                parent.to_path_buf()
            }
        })
        .unwrap_or_else(|| {
            if path == Path::new("") {
                this_dir()
            } else {
                path.to_path_buf()
            }
        })
}

/// Extracts the base directory of `path` and normalizes the value of `path` based
/// on the resulting base directory.
///
/// Returns a tuple whose first component is the normalized path and whose second
/// component is the base directory.
///
/// This function is equivalent to [`base_dir`] in determining the base directory.
pub fn extract_dir<P: AsRef<Path>>(path: P) -> (PathBuf, PathBuf) {
    let path = path.as_ref();
    if path.is_dir() {
        (path.to_path_buf(), path.to_path_buf())
    } else {
        if let Some(parent) = path.parent() {
            if parent == Path::new("") {
                let dir = this_dir();
                (dir.join(path), dir)
            } else {
                (path.to_path_buf(), parent.to_path_buf())
            }
        } else {
            if path == Path::new("") {
                let dir = this_dir();
                (dir.join(path), dir)
            } else {
                (path.to_path_buf(), path.to_path_buf())
            }
        }
    }
}

/// Returns a pretty version of `path` by attempting to strip the prefix if it matches
/// the value of [`home_dir`] and replacing it with `"~/"`, otherwise `path` itself is
/// returned.
pub fn pretty_path<P: AsRef<Path>>(path: P) -> String {
    let path = path.as_ref().as_string();
    path.strip_prefix(&home_dir().as_string())
        .map(|suffix| {
            if suffix.len() > 0 {
                String::from("~") + suffix
            } else {
                String::from("~/")
            }
        })
        .unwrap_or(path)
}

/// Returns `true` if `path` is a directory.
pub fn is_dir<P: AsRef<Path>>(path: P) -> bool {
    Path::new(path.as_ref()).is_dir()
}

/// Returns a lexicographically-sorted list of files and directories contained
/// in `dir`, quietly discarding any I/O errors when reading the directory.
pub fn list_dir<P: AsRef<Path>>(dir: P) -> Vec<PathBuf> {
    let mut entries = match dir.as_ref().read_dir() {
        Ok(entries) => entries
            .flat_map(|entry| entry.ok().map(|e| e.path()))
            .collect(),
        Err(_) => {
            vec![]
        }
    };
    entries.sort();
    entries
}

/// Returns the canonicalized form of `path`, or `path` itself if the canonicalization
/// failed for any reason.
pub fn canonicalize<P: AsRef<Path>>(path: P) -> PathBuf {
    path.as_ref()
        .canonicalize()
        .unwrap_or_else(|_| path.as_ref().to_path_buf())
}
