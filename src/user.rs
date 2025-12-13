//! A collection of types and implementations for interfacing with users.

use crate::env::Environment;
use crate::key::Key;
use crate::operation::Action;
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
    ///
    /// The default implementation returns `None`.
    fn prepare(&mut self) -> Option<String> {
        None
    }

    /// Allows the completer to evaluate the input `value` in its current form and
    /// return an optional hint.
    ///
    /// Under normal circumstances, this method is expected to be called with each
    /// change to the input, including insertion and removal of characters.
    ///
    /// The default implementation returns `None`.
    #[allow(unused_variables, reason = "retain expressiveness")]
    fn evaluate(&mut self, value: &str) -> Option<String> {
        None
    }

    /// Allows the completer to make a suggestion based on the input `value` in its
    /// current form and the directional nature of `suggest` by returning a tuple
    /// containing an optional replacement value and an optional hint.
    ///
    /// Under normal circumstances, this method is called only when a request is made
    /// by the user, such as pressing the TAB key (forward suggestion) or S-TAB key
    /// (backward suggestion).
    ///
    /// The default implementation returns `(None, None)`.
    #[allow(unused_variables, reason = "retain expressiveness")]
    fn suggest(&mut self, value: &str, suggest: Suggest) -> (Option<String>, Option<String>) {
        (None, None)
    }

    /// Allows the completer to accept or reject the input `value` in its current form,
    /// returning `Some` with a possibly altered final form, or `None` if the value is
    /// rejected.
    ///
    /// Under normal circumstances, this method is called only when the user requests
    /// that the input be accepted, such as pressing the RETURN key.
    ///
    /// The default implementation returns `Some(value)`.
    fn accept(&mut self, value: &str) -> Option<String> {
        Some(value.to_string())
    }
}

/// Captures the directional notion of suggestions.
pub enum Suggest {
    Forward,
    Backward,
}

/// Returns an implementation of [`Completer`] that essentially provides no assistance
/// whatsoever.
pub fn null_completer() -> Box<dyn Completer> {
    Box::new(NullCompleter)
}

/// Returns an implementation of [`Completer`] that accepts `y`es/`n`o input.
pub fn yes_no_completer() -> Box<dyn Completer> {
    Box::new(YesNoCompleter)
}

/// Returns an implementation of [`Completer`] that accepts `y`es/`n`o/`a`ll
/// input.
pub fn yes_no_all_completer() -> Box<dyn Completer> {
    Box::new(YesNoAllCompleter)
}

/// Returns an implementation of [`Completer`] that accepts numbers represented by
/// the given `radix` and in the range defined by `u32`.
pub fn number_completer(radix: u32) -> Box<dyn Completer> {
    Box::new(NumberCompleter::new(radix))
}

/// Parses the input `value` and returns a number represented by the given `radix`
/// if correctly formed, otherwise `None`.
pub fn number_parse(value: &str, radix: u32) -> Option<u32> {
    NumberCompleter::new(radix).parse(value)
}

/// Returns an implementation of [`Completer`] that accepts a _line_ number followed
/// by an optional _column_ number.
pub fn line_column_completer() -> Box<dyn Completer> {
    Box::new(LineColumnCompleter)
}

/// Parses the input `value` and returns a _line_ number and an optional _column_ number
/// if correctly formed, otherwise `None`.
pub fn line_column_parse(value: &str) -> Option<(u32, Option<u32>)> {
    LineColumnCompleter::parse(value)
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

impl Completer for NullCompleter {}

/// A completer that accepts case-insensitive values `"y"` and `"n"`, always yielding
/// accepted values in lowercase.
struct YesNoCompleter;

impl YesNoCompleter {
    fn hint() -> Option<String> {
        Some(" (y)es, (n)o".to_string())
    }

    fn parse(value: &str) -> Option<&'static str> {
        const ACCEPTED: [&str; 2] = ["y", "n"];
        let value = value.to_lowercase();
        ACCEPTED
            .iter()
            .position(|&s| s == value)
            .map(|i| ACCEPTED[i])
    }
}

impl Completer for YesNoCompleter {
    fn prepare(&mut self) -> Option<String> {
        Self::hint()
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        if Self::parse(value).is_some() {
            None
        } else {
            Self::hint()
        }
    }

    fn suggest(&mut self, _: &str, _: Suggest) -> (Option<String>, Option<String>) {
        (None, Self::hint())
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        Self::parse(value).map(|v| v.to_string())
    }
}

/// A completer that accepts case-insensitive values `"y"`, `"n"`, and `"a"`, always
/// yielding accepted values in lowercase.
struct YesNoAllCompleter;

impl YesNoAllCompleter {
    fn hint() -> Option<String> {
        Some(" (y)es, (n)o, (a)ll".to_string())
    }

    fn parse(value: &str) -> Option<&'static str> {
        const ACCEPTED: [&str; 3] = ["y", "n", "a"];
        let value = value.to_lowercase();
        ACCEPTED
            .iter()
            .position(|&s| s == value)
            .map(|i| ACCEPTED[i])
    }
}

impl Completer for YesNoAllCompleter {
    fn prepare(&mut self) -> Option<String> {
        Self::hint()
    }

    fn evaluate(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        if Self::parse(value).is_some() {
            None
        } else {
            Self::hint()
        }
    }

    fn suggest(&mut self, _: &str, _: Suggest) -> (Option<String>, Option<String>) {
        (None, Self::hint())
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        Self::parse(value).map(|v| v.to_string())
    }
}

/// A completer that accepts numbers in the range defined by `u32`.
struct NumberCompleter {
    radix: u32,
}

impl NumberCompleter {
    fn new(radix: u32) -> NumberCompleter {
        NumberCompleter { radix }
    }

    fn parse(&self, value: &str) -> Option<u32> {
        match u32::from_str_radix(value, self.radix) {
            Ok(n) => Some(n),
            Err(_) => None,
        }
    }
}

impl Completer for NumberCompleter {
    fn evaluate(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        if value.len() == 0 || self.parse(value).is_some() {
            None
        } else {
            Some(" (invalid)".to_string())
        }
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        self.parse(value).map(|_| value.to_string())
    }
}

/// A completer that accepts a _line_ number followed by an optional _column_ number.
struct LineColumnCompleter;

impl LineColumnCompleter {
    fn parse(value: &str) -> Option<(u32, Option<u32>)> {
        let vs = value
            .split(',')
            .map(|v| v.trim())
            .filter(|v| v.len() > 0)
            .collect::<Vec<_>>();

        match &vs[..] {
            [line] => match line.parse::<u32>() {
                Ok(l) => Some((l, None)),
                Err(_) => None,
            },
            [line, col] => match (line.parse::<u32>(), col.parse::<u32>()) {
                (Ok(l), Ok(c)) => Some((l, Some(c))),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Completer for LineColumnCompleter {
    fn evaluate(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        if value.len() == 0 || Self::parse(value).is_some() {
            None
        } else {
            Some(" (invalid)".to_string())
        }
    }

    fn accept(&mut self, value: &str) -> Option<String> {
        let value = value.trim();
        Self::parse(value).map(|_| value.to_string())
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

    fn suggest(&mut self, _: &str, suggest: Suggest) -> (Option<String>, Option<String>) {
        let count = self.matches.len();
        if count == 0 {
            (None, Some(" (no matches)".to_string()))
        } else if count == 1 {
            let replace = self.match_for(0).to_string();
            (Some(replace), None)
        } else {
            let index = if let Some(index) = self.last_match {
                match suggest {
                    Suggest::Forward => (index + 1) % count,
                    Suggest::Backward => (index + count - 1) % count,
                }
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

    fn suggest(&mut self, value: &str, suggest: Suggest) -> (Option<String>, Option<String>) {
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
                    match suggest {
                        Suggest::Forward => (index + 1) % count,
                        Suggest::Backward => (index + count - 1) % count,
                    }
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
}
