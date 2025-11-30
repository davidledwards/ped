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
    fn find(&self, buffer: &Buffer, pos: usize) -> Option<(usize, usize)>;

    /// Equivalent to [`find`](Self::find) with the exception that `buffer` is an
    /// `&str` type.
    fn find_str(&self, buffer: &str, pos: usize) -> Option<(usize, usize)>;
}

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
                // Find border of pattern[i..pat_len]
                while j <= len && pattern[i - 1] != pattern[j - 1] {
                    // If good_suffix[j-1] hasn't been set yet, set it
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

    pub fn new(term: &str, case_strict: bool) -> TermPattern {
        // Pattern is downcased when case-sensitivity is relaxed, otherwise
        // it is faithful represention of term.
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

    fn search(&self, buffer: &Buffer, pos: usize) -> Option<(usize, usize)> {
        let len = self.pattern.len();
        if len > 0 && pos + len <= buffer.size() {
            let stop_pos = buffer.size() - len;
            let mut pos = pos;
            while pos <= stop_pos {
                // Pattern matching occurs right-to-left.
                let mut i = len;
                while i > 0 && self.pattern[i - 1] == self.buf_at(buffer, pos + i - 1) {
                    i -= 1;
                }
                if i == 0 {
                    // Pattern matched.
                    return Some((pos, pos + len));
                } else {
                    let bc = self.buf_at(buffer, pos + i - 1);
                    let bc_shift = if let Some(&p) = self.bc_shift.get(&bc) {
                        if p < i - 1 { i - 1 - p } else { 1 }
                    } else {
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

    fn find(&self, buffer: &Buffer, pos: usize) -> Option<(usize, usize)> {
        self.search(buffer, pos).or_else(|| {
            if pos > 0 {
                self.search(buffer, 0)
            } else {
                None
            }
        })
    }

    fn find_str(&self, buffer: &str, pos: usize) -> Option<(usize, usize)> {
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

    fn search(&self, buffer: &str, pos: usize) -> Option<(usize, usize)> {
        // Convert starting position into an offset.
        let pos_offset = etc::pos_to_offset(buffer, pos);

        self.regex.find_at(buffer, pos_offset).map(|m| {
            // Convert starting and ending offsets into their respective character
            // positions.
            let Range { start, end } = m.range();
            let start_pos = etc::offset_to_pos(buffer, start);

            // This trick saves us from rescanning entire buffer to find ending offset.
            let end_pos = start_pos + etc::offset_to_pos(&buffer[start..], end - start);
            (start_pos, end_pos)
        })
    }
}

impl Pattern for RegexPattern {
    fn pattern(&self) -> &str {
        self.regex.as_str()
    }

    fn find(&self, buffer: &Buffer, pos: usize) -> Option<(usize, usize)> {
        // Entire buffer must be converted to string since regex library only works
        // with &str as opposed to iterators.
        let buf = buffer.iter().collect::<String>();
        self.find_str(&buf, pos)
    }

    fn find_str(&self, buffer: &str, pos: usize) -> Option<(usize, usize)> {
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
        let found = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(found, (10, 19));
    }

    #[test]
    fn search_term_normal_at_end() {
        let pattern = TermPattern::new("DOG", false);
        let found = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(found, (40, 43));
    }

    #[test]
    fn search_term_normal_not_found() {
        let pattern = TermPattern::new("jumpz", false);
        let found = pattern.find_str(TEXT, 0);
        assert_eq!(found, None);
    }

    #[test]
    fn search_term_case() {
        let pattern = TermPattern::new("jump", true);
        let found = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(found, (20, 24));
    }

    #[test]
    fn search_term_case_at_end() {
        let pattern = TermPattern::new("dog", false);
        let found = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(found, (40, 43));
    }

    #[test]
    fn search_term_case_not_found() {
        let pattern = TermPattern::new("Jump", true);
        let found = pattern.find_str(TEXT, 0);
        assert_eq!(found, None);
    }

    #[test]
    fn search_regex_normal() {
        let pattern = RegexPattern::new(build_regex_normal("qu[A-Z]+\\s*.+wN"));
        let found = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(found, (4, 15))
    }

    #[test]
    fn search_regex_normal_at_end() {
        let pattern = RegexPattern::new(build_regex_normal("LazY.*$"));
        let found = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(found, (35, 43))
    }

    #[test]
    fn search_regex_normal_not_found() {
        let pattern = RegexPattern::new(build_regex_normal("qu[a-z]+\\s.+nw"));
        let found = pattern.find_str(TEXT, 0);
        assert_eq!(found, None);
    }

    #[test]
    fn search_regex_case() {
        let pattern = RegexPattern::new(build_regex_case("qu[a-z]+\\s*.+wn"));
        let found = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(found, (4, 15))
    }

    #[test]
    fn search_regex_case_at_end() {
        let pattern = RegexPattern::new(build_regex_case("lazy.*$"));
        let found = pattern.find_str(TEXT, 0).unwrap();
        assert_eq!(found, (35, 43))
    }

    #[test]
    fn search_regex_case_not_found() {
        let pattern = RegexPattern::new(build_regex_case("qu[A-Z]+\\s.+wn"));
        let found = pattern.find_str(TEXT, 0);
        assert_eq!(found, None);
    }

    #[test]
    fn search_empty_buffer() {
        let pattern = TermPattern::new("anything", false);
        let found = pattern.find_str("", 0);
        assert_eq!(found, None);

        let pattern = RegexPattern::new(build_regex_normal("any.+thing"));
        let found = pattern.find_str("", 0);
        assert_eq!(found, None);
    }

    #[test]
    fn search_with_empty_term() {
        let pattern = TermPattern::new("", false);
        let found = pattern.find_str(TEXT, 0);
        assert_eq!(found, None);
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
