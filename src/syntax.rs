//! Contains everything related to syntax definitions and loading of external
//! configuration files.
//!
//! [`Syntax`] types are essentially collections of regular expressions that match
//! tokens for various kinds of file formats.
//!
//! External configuration files are expected to be formatted according to the
//! [TOML specification](https://toml.io).
//!
//! Syntax files are enumerated and loaded by searching predefined directories via
//! [`Registry::load()`], or from a custom directory using [`Registry::load_dir()`].
//! Any file in the applicable directory with an extension of `.toml` is assumed to
//! be a syntax configuration.
//!
//! The order of precedence for predefined directories follows:
//!
//! * `$HOME/.ped/syntax`
//! * `$HOME/.config/ped/syntax`

use crate::color::{ColorValue, Colors};
use crate::error::{Error, Result};
use crate::sys::{self, AsString};
use indexmap::IndexMap;
use regex_lite::{Captures, Regex, RegexBuilder};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::Path;

/// A registry of syntax configurations.
#[derive(Default)]
pub struct Registry {
    /// A map of canonical syntax names to syntax configurations.
    syntax_map: HashMap<String, Syntax>,

    /// A list of regular expressions that map to canonical syntax names.
    re_list: Vec<(Regex, String)>,
}

/// A syntax configuration.
#[derive(Clone)]
pub struct Syntax {
    /// The canonical name of the syntax configuration.
    pub name: String,

    /// A single regular expression aggregating all token definitions, each adorned
    /// with its own capture group name.
    pub re: Regex,

    /// A collection of token definitions whose order is crucial since [`re`](Self::re)
    /// is constructed in the order of iteration.
    tokens: Vec<Token>,
}

/// A token represents a regular expression with a unique identifier that is used in
/// forming capture group names.
#[derive(Clone)]
struct Token {
    /// A unique identifier representing this token.
    id: usize,

    /// The unique capture group name assigned to this token, which is formed using
    /// [`id`](Self::id).
    name: String,

    /// The regular expression for this token.
    pattern: String,

    /// The foreground color associated with this token.
    color: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalConfig {
    syntax: ExternalSyntax,
    tokens: Option<IndexMap<String, ColorValue>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalSyntax {
    name: String,
    files: Vec<String>,
}

impl Syntax {
    /// Name of default syntax.
    const DEFAULT_NAME: &str = "Text";

    /// A regular expression that never matches, which is used when no tokens are
    /// provided.
    const EMPTY_REGEX: &str = "^$a";

    /// Creates a new syntax identified by `name` and using `tokens`, which are
    /// tuples containing a regular expression and a foreground color.
    ///
    /// If any of the regular expressions are malformed or the aggregate size of all
    /// regular expressions is too large, then an error is returned.
    pub fn new(name: String, tokens: Vec<(String, u8)>) -> Result<Syntax> {
        // Tokens are adorned with capture group names of "_<id>" where <id> is the
        // index of the token definition offset by 1. Offset is required because
        // token id 0 is reserved to represent the absence of a token.
        let tokens = if tokens.len() > 0 {
            tokens
        } else {
            vec![(Self::EMPTY_REGEX.to_string(), 0)]
        };

        let tokens = tokens
            .iter()
            .enumerate()
            .map(|(i, (pattern, color))| Token {
                id: i + 1,
                name: format!("_{}", i + 1),
                pattern: pattern.clone(),
                color: *color,
            })
            .collect::<Vec<_>>();

        // Join all token regular expressions using capture group names.
        let pattern = tokens
            .iter()
            .map(|def| format!("(?<{}>{})", def.name, def.pattern))
            .collect::<Vec<_>>()
            .join("|");

        let re = match RegexBuilder::new(&pattern).multi_line(true).build() {
            Ok(re) => re,
            Err(e) => return Err(Error::invalid_regex(&pattern, &e)),
        };

        let this = Syntax { name, re, tokens };
        Ok(this)
    }

    /// Returns the token id and the byte offset range for the matching capture group
    /// `cap`.
    ///
    /// This function panics if the capture group does not match any of the expected
    /// names, as such condition would indicate a correctness problem.
    pub fn lookup(&self, cap: &Captures) -> (usize, Range<usize>) {
        self.tokens
            .iter()
            .find_map(|token| cap.name(&token.name).map(|m| (token.id, m.range())))
            .unwrap_or_else(|| panic!("{}: capture group expected for token", &cap[0]))
    }

    /// Returns the foreground color associated with the token referenced by `id`.
    pub fn color(&self, id: usize) -> Option<u8> {
        if id == 0 {
            None
        } else {
            Some(self.tokens[id - 1].color)
        }
    }
}

impl Default for Syntax {
    fn default() -> Syntax {
        Syntax::new(Self::DEFAULT_NAME.to_string(), Vec::new())
            .unwrap_or_else(|e| panic!("{}: syntax failed: {}", Self::DEFAULT_NAME, e))
    }
}

impl Registry {
    /// A collection of directories to try loading syntax configurations.
    const TRY_DIRS: [&str; 2] = [".ped/syntax", ".config/ped/syntax"];

    /// File extensions that identify candidate syntax configurations.
    const FILE_EXT: &str = "toml";

    /// Returns a syntax registry that is initialized using syntax configuration files
    /// from well-known directories.
    ///
    /// An empty registry is returned if none of the well-known directories exist or
    /// contain any configuration files.
    ///
    /// An error is returned if any syntax configuration file is malformed in any way.
    pub fn load(colors: &Colors) -> Result<Registry> {
        let root_path = sys::home_dir();
        Self::TRY_DIRS
            .iter()
            .map(|dir| root_path.join(dir))
            .find(|dir| sys::is_dir(dir))
            .map(|dir| Self::load_dir(dir, colors))
            .unwrap_or_else(|| Ok(Registry::default()))
    }

    /// Returns a syntax registry that is initialized using syntax configuration files
    /// in `dir`.
    ///
    /// An empty registry is returned if `dir` is missing or not a directory.
    ///
    /// An error is returned if any syntax configuration file is malformed in any way.
    pub fn load_dir<P: AsRef<Path>>(dir: P, colors: &Colors) -> Result<Registry> {
        if sys::is_dir(&dir) {
            Self::load_registry(&dir, colors)
        } else {
            Ok(Registry::default())
        }
    }

    /// Returns the first syntax configuration matching the file name of `path`, or
    /// `None` if no match is found.
    pub fn find<P: AsRef<Path>>(&self, path: P) -> Option<&Syntax> {
        path.as_ref().file_name().and_then(|file| {
            self.re_list
                .iter()
                .find(|(re, _)| re.is_match(&file.to_string_lossy()))
                .and_then(|(_, name)| self.syntax_map.get(name))
        })
    }

    /// Creates a registry by enumerating and loading files from `dir`.
    fn load_registry<P: AsRef<Path>>(dir: P, colors: &Colors) -> Result<Registry> {
        let paths = sys::list_files(dir.as_ref());
        let paths = paths
            .iter()
            .filter(|path| {
                if let Some(ext) = path.extension() {
                    ext == Self::FILE_EXT
                } else {
                    false
                }
            })
            .collect::<Vec<_>>();

        let mut syntax_map = HashMap::new();
        let mut re_list = Vec::new();
        for path in paths {
            let (syntax, res) = Self::load_syntax(path, colors)?;
            let name = syntax.name.clone();
            for re in res {
                re_list.push((re, name.clone()));
            }
            syntax_map.insert(name, syntax);
        }

        let registry = Registry {
            syntax_map,
            re_list,
        };
        Ok(registry)
    }

    /// Loads the syntax configuration referenced by `path`, returning the syntax
    /// along with a vector of regular expressions for matching file names.
    fn load_syntax<P: AsRef<Path>>(path: P, colors: &Colors) -> Result<(Syntax, Vec<Regex>)> {
        let config = Self::read_file(path.as_ref())?;

        // Build tokens and create syntax configuration.
        let tokens = if let Some(tokens) = config.tokens {
            let mut ts = Vec::new();
            for (pattern, color) in &tokens {
                if let Some(c) = colors.lookup_value(color) {
                    ts.push((pattern.clone(), c));
                } else {
                    return Err(Error::invalid_color(&color.to_string()));
                }
            }
            ts
        } else {
            Vec::new()
        };
        let syntax = Syntax::new(config.syntax.name, tokens)?;

        // Convert file patterns to regular expressions.
        let mut res = Vec::new();
        for pattern in config.syntax.files {
            let re = Regex::new(&pattern).map_err(|e| Error::invalid_regex(&pattern, &e))?;
            res.push(re);
        }
        Ok((syntax, res))
    }

    fn read_file(path: &Path) -> Result<ExternalConfig> {
        let content = fs::read_to_string(path).map_err(|e| Error::io(&path.as_string(), e))?;
        toml::from_str::<ExternalConfig>(&content).map_err(|e| Error::syntax(&path.as_string(), &e))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    const SYNTAX_NAME: &str = "foo";

    const SYNTAX_TOKENS: [(&str, u8); 3] = [
        (r#"-?\d+(?:\.\d+)?(?:[eE]-?\d+)?"#, 1),
        (r#""(?:[^"\\]|(?:\\.))*""#, 2),
        (r#"\b(?:foo|bar)\b"#, 3),
    ];

    #[test]
    fn new_syntax() {
        let syntax = build_syntax();
        assert_eq!(syntax.name, SYNTAX_NAME);
        assert_eq!(syntax.tokens.len(), SYNTAX_TOKENS.len());

        for (i, token) in syntax.tokens.iter().enumerate() {
            assert_eq!(token.id, i + 1);
            assert_eq!(token.name, format!("_{}", i + 1));
            assert_eq!(token.pattern, SYNTAX_TOKENS[i].0);
            assert_eq!(token.color, SYNTAX_TOKENS[i].1);
        }
    }

    #[test]
    fn new_syntax_empty() {
        let syntax = build_empty_syntax();
        assert_eq!(syntax.name, SYNTAX_NAME);
        assert_eq!(syntax.tokens.len(), 1);

        let token = &syntax.tokens[0];
        assert_eq!(token.id, 1);
        assert_eq!(token.name, "_1");
        assert_eq!(token.pattern, Syntax::EMPTY_REGEX);
        assert_eq!(token.color, 0);
    }

    #[test]
    fn invalid_token() {
        let tokens = vec![("(bad".to_string(), 0)];
        let syntax = Syntax::new(SYNTAX_NAME.to_string(), tokens);
        assert!(syntax.is_err());
    }

    pub fn build_syntax() -> Syntax {
        Syntax::new(SYNTAX_NAME.to_string(), build_tokens()).unwrap()
    }

    pub fn build_empty_syntax() -> Syntax {
        Syntax::new(SYNTAX_NAME.to_string(), Vec::new()).unwrap()
    }

    fn build_tokens() -> Vec<(String, u8)> {
        SYNTAX_TOKENS
            .iter()
            .map(|(token, color)| (token.to_string(), *color))
            .collect()
    }
}
