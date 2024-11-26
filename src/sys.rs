//! # System functions
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

pub fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(|path| PathBuf::from(path))
        .unwrap_or_else(|| this_dir())
}

pub fn working_dir() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| this_dir())
}

pub fn this_dir() -> PathBuf {
    PathBuf::from(".")
}

pub fn base_dir(path: &Path) -> PathBuf {
    path.parent()
        .map(|parent| {
            if parent == Path::new("") {
                this_dir()
            } else {
                parent.to_path_buf()
            }
        })
        .unwrap_or_else(|| this_dir())
}

pub fn extract_dir(path: &Path) -> (PathBuf, PathBuf) {
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
            let dir = this_dir();
            (dir.join(path), dir)
        }
    }
}

pub fn is_dir(path: &str) -> bool {
    Path::new(path).is_dir()
}

pub fn list_dir(dir: &Path) -> Vec<PathBuf> {
    let mut entries = match dir.read_dir() {
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
