//! Text searching.

use crate::buffer::Buffer;
use crate::etc;
use regex_lite::Regex;
use std::collections::HashMap;
use std::ops::Range;

/// Defines an interface for a pattern-matching algorithm.
pub trait Pattern {
    /// Returns the pattern.
    #[allow(dead_code, reason = "possible future use")]
    fn pattern(&self) -> &str;

    /// Searches `buffer` starting at `pos` for the first pattern match, returning a
    /// tuple comprised of the starting and ending positions of the match in `buffer`.
    ///
    /// Regardless of `pos`, implementations are required to perform a full search over
    /// `buffer`, which implies that the search will effectively wrap, if necessary,
    /// when `pos` > 0.
    ///
    /// A return value of `None` indicates that `buffer` does not contain a match for
    /// the pattern.
    fn find(&self, buffer: &Buffer, pos: usize) -> Option<Match>;

    /// Equivalent to [`find`](Self::find) with the exception that `buffer` is an
    /// `&str` type.
    fn find_str(&self, buffer: &str, pos: usize) -> Option<Match>;
}

/// Represents a pattern match, where the first value is the _starting_ buffer
/// position and the second value is the _ending_ buffer position.
pub struct Match(pub usize, pub usize);

/// Returns a pattern-matching algorithm using `term` as the search string, and
/// `case_strict` to indicate the sensitivity of case when searching.
pub fn using_term(term: &str, case_strict: bool) -> Box<dyn Pattern> {
    Box::new(TermPattern::new(term, case_strict))
}

/// Returns a pattern-matching algorithm using `regex` as the regular expression.
pub fn using_regex(regex: Regex) -> Box<dyn Pattern> {
    Box::new(RegexPattern::new(regex))
}

/// A term-oriented pattern-matching algorithm implemented using the Boyer-Moore
/// algorithm.
///
/// The most efficient method of search is [`find()`](Pattern::find) because the
/// algorithm is able to work directly with [`Buffer`]s. Using
/// [`find_str()`](Pattern::find_str) requires an intermediate conversion from
/// `&str` to [`Buffer`].
struct TermPattern {
    /// The term provided during construction.
    #[allow(dead_code, reason = "possible future use")]
    term: String,

    /// A patternized version of [`term`](Self::term).
    pattern: Vec<char>,

    /// Bad character shift table maps characters in `pattern` to their rightmost
    /// position.
    bc_shift: HashMap<char, usize>,

    /// Good suffix shift table determines how far to shift based on position in
    /// `pattern`.
    gs_shift: Vec<usize>,

    /// Indicates the sensitivity of case.
    case_strict: bool,
}

impl TermPattern {
    pub fn new(term: &str, case_strict: bool) -> TermPattern {
        // Pattern is downcased when case-sensitivity is relaxed, otherwise
        // it will be faithful represention of term.
        let pattern = term
            .chars()
            .map(|c| {
                if case_strict {
                    c
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect::<Vec<_>>();

        // Construct shift tables.
        let bc_shift = Self::build_bc_shift(&pattern);
        let gs_shift = Self::build_gs_shift(&pattern);

        TermPattern {
            term: term.to_string(),
            pattern,
            bc_shift,
            gs_shift,
            case_strict,
        }
    }

    fn build_bc_shift(pattern: &[char]) -> HashMap<char, usize> {
        let len = pattern.len();
        pattern
            .iter()
            .enumerate()
            .take(len.saturating_sub(1))
            .map(|(i, c)| (*c, i))
            .collect()
    }

    fn build_gs_shift(pattern: &[char]) -> Vec<usize> {
        let len = pattern.len();
        if len > 0 {
            let mut gs_shift = vec![len; len];
            let mut border = vec![0; len + 1];
            border[len] = len + 1;
            let mut i = len;
            let mut j = len + 1;
            while i > 0 {
                while j <= len && pattern[i - 1] != pattern[j - 1] {
                    if gs_shift[j - 1] == len {
                        gs_shift[j - 1] = j - i;
                    }
                    j = border[j];
                }
                i -= 1;
                j -= 1;
                border[i] = j;
            }
            j = border[0];
            for i in 0..len {
                if gs_shift[i] == len {
                    gs_shift[i] = j;
                }
                if i == j {
                    j = border[j];
                }
            }
            gs_shift
        } else {
            vec![]
        }
    }

    fn search(&self, buffer: &Buffer, pos: usize) -> Option<Match> {
        let len = self.pattern.len();
        if len > 0 && pos + len <= buffer.size() {
            // Since Boyer-Moore searches backwards relative to pattern, this is the
            // position in buffer at which searching should stop, otherwise pattern
            // would extend beyond end of buffer.
            let stop_pos = buffer.size() - len;

            // Search until first is found or stop position is reached.
            let mut pos = pos;
            while pos <= stop_pos {
                // Pattern matching occurs right-to-left, so keep matching characters
                // until pattern is exhausted or mismatch occurs.
                let mut i = len;
                while i > 0 && self.pattern[i - 1] == self.buf_at(buffer, pos + i - 1) {
                    i -= 1;
                }
                if i == 0 {
                    // Pattern matched.
                    return Some(Match(pos, pos + len));
                } else {
                    // Bad character that did not match pattern.
                    let bc = self.buf_at(buffer, pos + i - 1);

                    // Calculate shift distance using bad character shift table.
                    let bc_shift = if let Some(&p) = self.bc_shift.get(&bc) {
                        // Shift distance must be at least 1.
                        (i - 1).saturating_sub(p).max(1)
                    } else {
                        // Bad character does not exist in pattern, so shift past
                        // remaining characters in pattern.
                        i
                    };
                    let gs_shift = self.gs_shift[i - 1];
                    pos += bc_shift.max(gs_shift);
                }
            }
            // At this point, buffer was exhausted without match.
            None
        } else {
            // Short-circuit when obvious that pattern will not match.
            None
        }
    }

    #[inline(always)]
    fn buf_at(&self, buffer: &Buffer, pos: usize) -> char {
        let c = buffer[pos];
        if self.case_strict {
            c
        } else {
            c.to_ascii_lowercase()
        }
    }
}

impl Pattern for TermPattern {
    fn pattern(&self) -> &str {
        &self.term
    }

    fn find(&self, buffer: &Buffer, pos: usize) -> Option<Match> {
        self.search(buffer, pos).or_else(|| {
            if pos > 0 {
                self.search(buffer, 0)
            } else {
                None
            }
        })
    }

    fn find_str(&self, buffer: &str, pos: usize) -> Option<Match> {
        let mut buf = Buffer::new();
        buf.insert_str(buffer);
        self.find(&buf, pos)
    }
}

/// A regex-oriented pattern-matching algorithm.
///
/// The most efficient method of search is [`find_str()`](Pattern::find_str) because
/// the algorithm only works directly with `&str` types. Using [`find()`](Pattern::find)
/// requires an intermediate conversion from [`Buffer`] to `&str`.
struct RegexPattern {
    regex: Regex,
}

impl RegexPattern {
    fn new(regex: Regex) -> RegexPattern {
        RegexPattern { regex }
    }

    fn search(&self, buffer: &str, pos: usize) -> Option<Match> {
        // Convert starting position into an offset.
        let pos_offset = etc::pos_to_offset(buffer, pos);

        self.regex.find_at(buffer, pos_offset).map(|m| {
            // Convert starting and ending offsets into their respective character
            // positions.
            let Range { start, end } = m.range();
            let start_pos = etc::offset_to_pos(buffer, start);

            // This trick saves us from rescanning entire buffer to find ending offset.
            let end_pos = start_pos + etc::offset_to_pos(&buffer[start..], end - start);
            Match(start_pos, end_pos)
        })
    }
}

impl Pattern for RegexPattern {
    fn pattern(&self) -> &str {
        self.regex.as_str()
    }

    fn find(&self, buffer: &Buffer, pos: usize) -> Option<Match> {
        // Entire buffer must be converted to string since regex library only works
        // with &str as opposed to iterators.
        let buf = buffer.iter().collect::<String>();
        self.find_str(&buf, pos)
    }

    fn find_str(&self, buffer: &str, pos: usize) -> Option<Match> {
        self.search(buffer, pos).or_else(|| {
            if pos > 0 {
                self.search(buffer, 0)
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex_lite::RegexBuilder;

    const TEXT: &str = "The quick brown fox jumps over the lazy dog";

    #[test]
    fn search_term_normal() {
        let pattern = TermPattern::new("BrOwN FoX", false);
        let Match(start_pos, end_pos) = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(start_pos, 10);
        assert_eq!(end_pos, 19);
    }

    #[test]
    fn search_term_normal_at_end() {
        let pattern = TermPattern::new("DOG", false);
        let Match(start_pos, end_pos) = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(start_pos, 40);
        assert_eq!(end_pos, 43);
    }

    #[test]
    fn search_term_normal_not_found() {
        let pattern = TermPattern::new("jumpz", false);
        let found = pattern.find_str(TEXT, 0);
        assert!(found.is_none());
    }

    #[test]
    fn search_term_case() {
        let pattern = TermPattern::new("jump", true);
        let Match(start_pos, end_pos) = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(start_pos, 20);
        assert_eq!(end_pos, 24);
    }

    #[test]
    fn search_term_case_at_end() {
        let pattern = TermPattern::new("dog", false);
        let Match(start_pos, end_pos) = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(start_pos, 40);
        assert_eq!(end_pos, 43);
    }

    #[test]
    fn search_term_case_not_found() {
        let pattern = TermPattern::new("Jump", true);
        let found = pattern.find_str(TEXT, 0);
        assert!(found.is_none());
    }

    #[test]
    fn search_regex_normal() {
        let pattern = RegexPattern::new(build_regex_normal("qu[A-Z]+\\s*.+wN"));
        let Match(start_pos, end_pos) = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(start_pos, 4);
        assert_eq!(end_pos, 15);
    }

    #[test]
    fn search_regex_normal_at_end() {
        let pattern = RegexPattern::new(build_regex_normal("LazY.*$"));
        let Match(start_pos, end_pos) = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(start_pos, 35);
        assert_eq!(end_pos, 43);
    }

    #[test]
    fn search_regex_normal_not_found() {
        let pattern = RegexPattern::new(build_regex_normal("qu[a-z]+\\s.+nw"));
        let found = pattern.find_str(TEXT, 0);
        assert!(found.is_none());
    }

    #[test]
    fn search_regex_case() {
        let pattern = RegexPattern::new(build_regex_case("qu[a-z]+\\s*.+wn"));
        let Match(start_pos, end_pos) = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(start_pos, 4);
        assert_eq!(end_pos, 15);
    }

    #[test]
    fn search_regex_case_at_end() {
        let pattern = RegexPattern::new(build_regex_case("lazy.*$"));
        let Match(start_pos, end_pos) = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(start_pos, 35);
        assert_eq!(end_pos, 43);
    }

    #[test]
    fn search_regex_case_not_found() {
        let pattern = RegexPattern::new(build_regex_case("qu[A-Z]+\\s.+wn"));
        let found = pattern.find_str(TEXT, 0);
        assert!(found.is_none());
    }

    #[test]
    fn search_empty_buffer() {
        let pattern = TermPattern::new("anything", false);
        let found = pattern.find_str("", 0);
        assert!(found.is_none());

        let pattern = RegexPattern::new(build_regex_normal("any.+thing"));
        let found = pattern.find_str("", 0);
        assert!(found.is_none());
    }

    #[test]
    fn search_with_empty_term() {
        let pattern = TermPattern::new("", false);
        let found = pattern.find_str(TEXT, 0);
        assert!(found.is_none());
    }

    fn build_regex_normal(term: &str) -> Regex {
        build_regex(term, false)
    }

    fn build_regex_case(term: &str) -> Regex {
        build_regex(term, true)
    }

    fn build_regex(term: &str, case_strict: bool) -> Regex {
        RegexBuilder::new(term)
            .case_insensitive(!case_strict)
            .build()
            .ok()
            .unwrap()
    }
}
