//! Syntax configuration.

use crate::color::Color;
use crate::error::{Error, Result};
use regex_lite::{Captures, Regex, RegexBuilder};
use std::collections::HashMap;
use std::ops::Range;

pub struct Library {
    syntax_map: HashMap<String, Syntax>,
    ext_map: HashMap<String, String>,
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

    /// The color associated with this token.
    color: Color,
}

impl Syntax {
    /// Creates a new syntax identified by `name` and using `tokens`, which are
    /// tuples containing a regular expression and a color.
    ///
    /// If any of the regular expressions are malformed or the aggregate size of all
    /// regular expressions is too large, then an error is returned.
    pub fn new(name: String, tokens: Vec<(String, Color)>) -> Result<Syntax> {
        // Tokens are adorned with capture group names of "_<id>" where <id> is the
        // index of the token definition offset by 1. Offset is required because
        // token id 0 is reserved to represent the absence of a token.
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

    /// Returns the color associated with the token referenced by `id`.
    pub fn color(&self, id: usize) -> Option<Color> {
        if id == 0 {
            None
        } else {
            Some(self.tokens[id - 1].color)
        }
    }
}

impl Library {
    pub fn load() -> Result<Library> {
        // todo
        // - load TOML file

        let this = Library {
            syntax_map: HashMap::new(),
            ext_map: HashMap::new(),
        };
        Ok(this)
    }

    pub fn find(&self, ext: &str) -> Option<&Syntax> {
        self.ext_map
            .get(ext)
            .and_then(|name| self.syntax_map.get(name))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    const NAME: &str = "foo";

    const TOKENS: [(&str, Color); 3] = [
        (r#"-?\d+(?:\.\d+)?(?:[eE]-?\d+)?"#, Color::new(1, 1)),
        (r#""(?:[^"\\]|(?:\\.))*""#, Color::new(2, 2)),
        (r#"\b(?:foo|bar)\b"#, Color::new(3, 3)),
    ];

    #[test]
    fn new_syntax() {
        let syntax = build_syntax();
        assert_eq!(syntax.name, NAME);
        assert_eq!(syntax.tokens.len(), TOKENS.len());
        for (i, token) in syntax.tokens.iter().enumerate() {
            assert_eq!(token.id, i + 1);
            assert_eq!(token.name, format!("_{}", i + 1));
            assert_eq!(token.pattern, TOKENS[i].0);
            assert_eq!(token.color, TOKENS[i].1);
        }
    }

    #[test]
    fn new_syntax_empty() {
        let syntax = Syntax::new(NAME.to_string(), Vec::new()).unwrap();
        assert_eq!(syntax.name, NAME);
        assert_eq!(syntax.tokens.len(), 0);
    }

    #[test]
    fn invalid_token() {
        let tokens = vec![("(bad".to_string(), Color::ZERO)];
        let syntax = Syntax::new(NAME.to_string(), tokens);
        assert!(syntax.is_err());
    }

    pub fn build_syntax() -> Syntax {
        Syntax::new(NAME.to_string(), build_tokens()).unwrap()
    }

    fn build_tokens() -> Vec<(String, Color)> {
        TOKENS
            .iter()
            .map(|(token, color)| (token.to_string(), *color))
            .collect()
    }
}
