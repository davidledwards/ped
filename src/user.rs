//! # User interaction
use crate::env::Environment;
use crate::op::Action;
use std::path::{Path, PathBuf};

/// Defines an interface for coordinating the solicitation of input from a user.
pub trait Inquirer {
    /// Returns the prompt displayed to the user.
    fn prompt(&self) -> String;

    /// Returns the [`Completer`] implementation attached to the inquirer.
    fn completer(&self) -> Box<dyn Completer>;

    /// Delegates processing of the user-provided response in `value`, returning an
    /// action to be taken by the controller.
    ///
    /// A `value` of `None` indicates that the inquiry was cancelled by the user.
    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action>;
}

/// Defines an interface for a versatile completion assistant when soliciting input
/// from a user.
pub trait Completer {
    /// Initializes the completer and returns an optional hint.
    fn prepare(&mut self) -> Option<String>;

    /// Allows the completer to evaluate the input `value` in its current form and
    /// return an optional hint.
    ///
    /// Under normal circumstances, this method is expected to be called with each
    /// change by the user, including insertion and removal of characters.
    fn evaluate(&mut self, value: &str) -> Option<String>;

    /// Allows the completer to make a suggestion based on the input `value` in its
    /// current form by returning a tuple containing an optional replacement value and
    /// an optional hint.
    ///
    /// Under normal circumstances, this method is called only when a request is made
    /// by the user, such as pressing the TAB key.
    fn suggest(&mut self, value: &str) -> (Option<String>, Option<String>);

    /// Allows the completer to accept, or not, the input `value` in its current form,
    /// returning `Some` with a possibly altered final form, or `None` if the value is
    /// rejected.
    ///
    /// Under normal circumstances, this method is called only when the user requests
    /// that the input be accepted, such as pressing the RETURN key.
    fn accept(&mut self, value: &str) -> Option<String>;
}

/// Returns an implementation of [`Completer`] that essentially provides no assistance
/// whatsoever.
pub fn null_completer() -> Box<dyn Completer> {
    Box::new(NullCompleter)
}

/// Returns an implementation of [`Completer`] that accepts `y`es/`n`o input.
pub fn yes_no_completer() -> Box<dyn Completer> {
    Box::new(YesNoCompleter::new())
}

/// Returns an implementation of [`Completer`] that accepts `y`es/`n`o/`a`ll
/// input.
pub fn yes_no_all_completer() -> Box<dyn Completer> {
    Box::new(YesNoAllCompleter::new())
}

/// Returns an implementation of [`Completer`] that accepts numbers in the range
/// defined by `u32`.
pub fn number_completer() -> Box<dyn Completer> {
    Box::new(NumberCompleter::new())
}

pub fn file_completer() -> Box<dyn Completer> {
    Box::new(FileCompleter::new())
}

/// A completer that does nothing.
struct NullCompleter;

impl Completer for NullCompleter {
    fn prepare(&mut self) -> Option<String> {
        None
    }

    fn evaluate(&mut self, _: &str) -> Option<String> {
        None
    }

    fn suggest(&mut self, _: &str) -> (Option<String>, Option<String>) {
        (None, None)
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        Some(value.to_string())
    }
}

/// A completer that accepts case-insensitive values `"y"` and `"n"`, always yielding
/// accepted values in lowercase.
struct YesNoCompleter {
    hint: Option<String>,
}

impl YesNoCompleter {
    const HINT: &str = " (y)es, (n)o";
    const ACCEPTED: [&str; 2] = ["y", "n"];

    fn new() -> YesNoCompleter {
        YesNoCompleter {
            hint: Some(Self::HINT.to_string()),
        }
    }
}

impl Completer for YesNoCompleter {
    fn prepare(&mut self) -> Option<String> {
        self.hint.clone()
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        if Self::ACCEPTED.contains(&value.to_lowercase().as_ref()) {
            None
        } else {
            self.hint.clone()
        }
    }

    fn suggest(&mut self, _: &str) -> (Option<String>, Option<String>) {
        (None, self.hint.clone())
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        if Self::ACCEPTED.contains(&value) {
            Some(value.to_lowercase())
        } else {
            None
        }
    }
}

/// A completer that accepts case-insensitive values `"y"`, `"n"`, and `"a"`, always
/// yielding accepted values in lowercase.
struct YesNoAllCompleter {
    hint: Option<String>,
}

impl YesNoAllCompleter {
    const HINT: &str = " (y)es, (n)o, (a)ll";
    const ACCEPTED: [&str; 3] = ["y", "n", "a"];

    fn new() -> YesNoAllCompleter {
        YesNoAllCompleter {
            hint: Some(Self::HINT.to_string()),
        }
    }
}

impl Completer for YesNoAllCompleter {
    fn prepare(&mut self) -> Option<String> {
        self.hint.clone()
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        if Self::ACCEPTED.contains(&value.to_lowercase().as_ref()) {
            None
        } else {
            self.hint.clone()
        }
    }

    fn suggest(&mut self, _: &str) -> (Option<String>, Option<String>) {
        (None, self.hint.clone())
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        if Self::ACCEPTED.contains(&value) {
            Some(value.to_lowercase())
        } else {
            None
        }
    }
}

/// A completer that accepts numbers in the range defined by `u32`.
pub struct NumberCompleter {
    hint: Option<String>,
}

impl NumberCompleter {
    const HINT: &str = " (enter number)";

    pub fn new() -> NumberCompleter {
        NumberCompleter {
            hint: Some(Self::HINT.to_string()),
        }
    }
}

impl Completer for NumberCompleter {
    fn prepare(&mut self) -> Option<String> {
        None
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        if value.trim().len() == 0 {
            None
        } else if let Ok(_) = value.trim().parse::<u32>() {
            None
        } else {
            self.hint.clone()
        }
    }

    fn suggest(&mut self, _: &str) -> (Option<String>, Option<String>) {
        (None, None)
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        if let Ok(n) = value.trim().parse::<u32>() {
            Some(n.to_string())
        } else {
            None
        }
    }
}

struct FileCompleter {
    completions: Vec<String>,
}

impl FileCompleter {
    fn new() -> FileCompleter {
        FileCompleter {
            completions: Vec::new(),
        }
    }

    fn find_matches(&self, prefix: &str) -> Vec<String> {
        self.completions
            .iter()
            .filter(|p| p.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Splits the `path` prefix into a normalized prefix and its directory component.
    ///
    /// The normalized prefix, returned as the first tuple value, may be different than
    /// the value of `path` due to some nuances in file enumeration. Therefore, the
    /// normalized prefix must be used when matching candidates.
    fn split_path(path: &str) -> (String, PathBuf) {
        fn no_parent(path: &Path) -> (PathBuf, PathBuf) {
            let dir = PathBuf::from(".");
            let mut prefix = dir.clone();
            prefix.push(path);
            (prefix, dir)
        }
        let path = Path::new(path);
        let (prefix, dir) = if path.is_dir() {
            (path.to_path_buf(), path.to_path_buf())
        } else {
            match path.parent() {
                Some(parent) if parent == Path::new("") => no_parent(path),
                Some(parent) => (path.to_path_buf(), parent.to_path_buf()),
                None => no_parent(path),
            }
        };
        (prefix.display().to_string(), dir)
    }

    fn get_completions(dir: &Path) -> Vec<String> {
        match dir.read_dir() {
            Ok(entries) => entries
                .flat_map(|entry| entry.ok().map(|e| e.path().display().to_string()))
                .collect(),
            Err(_) => {
                vec![]
            }
        }
    }
}

impl Completer for FileCompleter {
    fn prepare(&mut self) -> Option<String> {
        None
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        let (prefix, _) = Self::split_path(value);
        let matches = self.find_matches(&prefix);
        if matches.len() == 1 {
            matches[0].strip_prefix(&prefix).map(|s| s.to_string())
        } else {
            None
        }
    }

    fn suggest(&mut self, value: &str) -> (Option<String>, Option<String>) {
        let (prefix, dir) = Self::split_path(value);
        self.completions = Self::get_completions(&dir);

        let matches = self.find_matches(&prefix);

        match matches.len() {
            0 => (None, Some(" (no matches)".to_string())),
            1 => (Some(matches[0].clone()), None),
            n => (None, Some(format!(" ({n} matches)"))),
        }
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        Some(value.to_string())
    }
}
