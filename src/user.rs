//! A collection of types and implementations for interfacing with users.

use crate::env::Environment;
use crate::key::Key;
use crate::op::Action;
use crate::sys::{self, AsString};
use std::path::{Path, PathBuf};

/// Defines an interface for coordinating input from a user.
pub trait Question {
    /// Returns the prompt displayed to the user.
    fn prompt(&self) -> String;

    /// Returns an optional value for initializing the input.
    ///
    /// The default implementation returns `None`.
    fn value(&self) -> Option<String> {
        None
    }

    /// Returns the [`Completer`] implementation attached to the question.
    ///
    /// The default implementation returns a [`null_completer()`].
    fn completer(&self) -> Box<dyn Completer> {
        null_completer()
    }

    /// Allows the question to react to a partial input `value` following the
    /// processing of `key` that is not yet committed or cancelled, returning an
    /// optional _hint_.
    ///
    /// The default implementation does nothing and returns `None`.
    #[allow(unused_variables, reason = "retain expressiveness")]
    fn react(&mut self, env: &mut Environment, value: &str, key: &Key) -> Option<String> {
        None
    }

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

/// Returns an implementation of [`Completer`] that accepts numbers represented by
/// the given `radix` and in the range defined by `u32`.
pub fn number_completer(radix: u32) -> Box<dyn Completer> {
    Box::new(NumberCompleter::new(radix))
}

/// Returns an implementation of [`Completer`] that accepts a finite collection of
/// strings and provides searchability over the collection.
pub fn list_completer(accepted: Vec<String>) -> Box<dyn Completer> {
    Box::new(ListCompleter::new(accepted))
}

/// Returns an implementation of [`Completer`] that navigates files and directories.
pub fn file_completer(dir: PathBuf) -> Box<dyn Completer> {
    Box::new(FileCompleter::new(dir))
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
    radix: u32,
}

impl NumberCompleter {
    const INVALID_HINT: &str = " (invalid)";

    pub fn new(radix: u32) -> NumberCompleter {
        NumberCompleter { radix }
    }
}

impl Completer for NumberCompleter {
    fn prepare(&mut self) -> Option<String> {
        None
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        if value.len() == 0 || u32::from_str_radix(value, self.radix).is_ok() {
            None
        } else {
            Some(Self::INVALID_HINT.to_string())
        }
    }

    fn suggest(&mut self, _: &str) -> (Option<String>, Option<String>) {
        (None, None)
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        if u32::from_str_radix(value, self.radix).is_ok() {
            Some(value.to_string())
        } else {
            None
        }
    }
}

/// A completer that accepts a finite collection of strings and provides searchability
/// over the collection.
struct ListCompleter {
    accepted: Vec<String>,
    matches: Vec<usize>,
    last_match: Option<usize>,
}

impl ListCompleter {
    fn new(accepted: Vec<String>) -> ListCompleter {
        ListCompleter {
            accepted,
            matches: Vec::new(),
            last_match: None,
        }
    }

    fn refresh(&mut self, value: &str) -> usize {
        self.matches = self
            .accepted
            .iter()
            .enumerate()
            .filter(|(_, v)| v.to_lowercase().contains(&value.to_lowercase()))
            .map(|(index, _)| index)
            .collect();
        self.last_match = None;
        self.matches.len()
    }

    fn match_for(&self, index: usize) -> &String {
        &self.accepted[self.matches[index]]
    }
}

impl Completer for ListCompleter {
    fn prepare(&mut self) -> Option<String> {
        let count = self.refresh("");
        let hint = if count == 1 {
            format!(" ({})", self.match_for(0))
        } else {
            format!(" ({count} available)")
        };
        Some(hint)
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        let count = self.refresh(value);
        let hint = if count == 0 {
            " (no matches)".to_string()
        } else if count == 1 {
            format!(" ({})", self.match_for(0))
        } else {
            format!(" ({count} matches)")
        };
        Some(hint)
    }

    fn suggest(&mut self, _: &str) -> (Option<String>, Option<String>) {
        let count = self.matches.len();
        if count == 0 {
            (None, Some(" (no matches)".to_string()))
        } else if count == 1 {
            let replace = self.match_for(0).to_string();
            (Some(replace), None)
        } else {
            let index = if let Some(index) = self.last_match {
                (index + 1) % count
            } else {
                0
            };
            self.last_match = Some(index);
            let replace = self.match_for(index).to_string();
            let hint = format!(" ({} of {count} matches)", index + 1);
            (Some(replace), Some(hint))
        }
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        let value = value.to_string();
        if self.accepted.contains(&value) {
            Some(value)
        } else {
            None
        }
    }
}

/// A completer that provides assistance in navigating files and directories.
struct FileCompleter {
    dir: PathBuf,
    comp_dir: PathBuf,
    comps: Vec<String>,
    matches: Vec<String>,
    last_match: Option<usize>,
}

impl FileCompleter {
    fn new(dir: PathBuf) -> FileCompleter {
        FileCompleter {
            dir: dir.clone(),
            comp_dir: dir,
            comps: Vec::new(),
            matches: Vec::new(),
            last_match: None,
        }
    }

    fn refresh(&mut self, value: &str) -> (PathBuf, PathBuf) {
        let path = self.dir.join(value);
        let (prefix, dir) = sys::extract_dir(&path);

        // Comparing strings, as opposed to paths, is necessary because notion of
        // equality with respect to paths is more relaxed. For example, paths of
        // "foo/" and "foo/." are equivalent, yet completions generated from each
        // do not have equivalent strings.
        if self.comp_dir.as_string() != dir.as_string() {
            self.comp_dir = dir.clone();
            self.comps = Self::completions(&self.comp_dir);
        }

        // Always generate new matches regardless of whether completions changed.
        self.matches = Self::matches(&self.comps, &prefix.as_string());
        self.last_match = None;
        (prefix, dir)
    }

    fn replace_match(&mut self, index: usize) -> String {
        let dir = self.dir.as_string();
        let path = &self.matches[index];
        path.strip_prefix(&dir)
            .map(|suffix| {
                if let Some(suffix) = suffix.strip_prefix("/") {
                    suffix.to_string()
                } else {
                    suffix.to_string()
                }
            })
            .unwrap_or(path.to_string())
    }

    fn completions(dir: &Path) -> Vec<String> {
        sys::list_dir(dir)
            .iter()
            .map(|path| path.as_string())
            .collect()
    }

    fn matches(comps: &[String], prefix: &str) -> Vec<String> {
        comps
            .iter()
            .filter(|path| path.to_lowercase().starts_with(&prefix.to_lowercase()))
            .cloned()
            .collect()
    }
}

impl Completer for FileCompleter {
    fn prepare(&mut self) -> Option<String> {
        let (prefix, _) = sys::extract_dir(&self.comp_dir);
        self.comps = Self::completions(&self.comp_dir);
        self.matches = Self::matches(&self.comps, &prefix.as_string());
        None
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        // Return suffix of path when single match exists, which gives user opportunity
        // to follow with suggestion to replace input.
        let (prefix, _) = self.refresh(value);
        if self.matches.len() == 1 {
            // Cannot use strip_prefix() because it is case-sensitive and our matching
            // logic is case-insensitive.
            self.matches[0]
                .char_indices()
                .nth(prefix.as_string().chars().count())
                .map(|(i, _)| self.matches[0][i..].to_string())
        } else {
            None
        }
    }

    fn suggest(&mut self, value: &str) -> (Option<String>, Option<String>) {
        if value == "~/" {
            // A special case where input value references home directory, which is
            // not recognized by file system operations. In this case, the value is
            // replaced with home directory, ensuring that / is appended. Completions
            // and matches are refreshed as side effect.
            let replace = sys::home_dir().join("").as_string();
            self.refresh(&replace);
            (Some(replace), None)
        } else {
            let count = self.matches.len();
            if count == 0 {
                (None, Some(" (no matches)".to_string()))
            } else if count == 1 {
                // Replace input value when single match exists.
                let replace = self.replace_match(0);

                // Hint is provided when match is also directory, as this gives visual
                // cue to user that typing / will navigate into directory.
                let hint = if sys::is_dir(&self.matches[0]) {
                    Some("/".to_string())
                } else {
                    None
                };
                (Some(replace), hint)
            } else {
                // Keep track of index when scrolling through matches, though note this
                // is only necessary when number of matches more than one.
                let index = if let Some(index) = self.last_match {
                    (index + 1) % count
                } else {
                    0
                };
                self.last_match = Some(index);

                // Replace input value with most recent suggestion from list of matches.
                let replace = self.replace_match(index);

                // Hint not only appends / for matches that are directories, but also
                // includes indication of current position in total number of matches.
                let hint = format!(
                    "{} ({} of {count} matches)",
                    if sys::is_dir(&self.matches[index]) {
                        "/"
                    } else {
                        ""
                    },
                    index + 1
                );
                (Some(replace), Some(hint))
            }
        }
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        Some(value.to_string())
    }
}
