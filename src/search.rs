//! Text searching.

use crate::buffer::Buffer;
use crate::etc;
use regex_lite::Regex;
use std::collections::HashMap;
use std::ops::Range;

/// Defines an interface for a pattern-matching algorithm.
pub trait Pattern {
    /// Returns the pattern.
    #[allow(dead_code)]
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
}

/// Returns a pattern-matching algorithm using `term` as the search string, and
/// `case_strict` to indicate the sensitivity of case when searching.
pub fn using_term(term: String, case_strict: bool) -> Box<dyn Pattern> {
    Box::new(TermPattern::new(term, case_strict))
}

/// Returns a pattern-matching algorithm using `regex` as the regular expression.
pub fn using_regex(regex: Regex) -> Box<dyn Pattern> {
    Box::new(RegexPattern::new(regex))
}

/// A term-oriented pattern-matching algorithm implemented using the Boyer-Moore
/// algorithm.
struct TermPattern {
    /// The term provided during construction.
    term: String,

    /// A patternized version of [`term`](Self::term).
    pattern: Vec<char>,

    /// The shift table used by Boyer-Moore.
    shift: HashMap<char, usize>,

    /// Indicates the sensitivity of case.
    case_strict: bool,
}

impl TermPattern {
    pub fn new(term: String, case_strict: bool) -> TermPattern {
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

        // Shift table reflects case-sensitivity as well.
        let mut shift = HashMap::new();
        for (i, c) in pattern.iter().enumerate() {
            let c = if case_strict {
                *c
            } else {
                c.to_ascii_lowercase()
            };
            shift.insert(c, i);
        }

        TermPattern {
            term,
            pattern,
            shift,
            case_strict,
        }
    }

    fn search(&self, buffer: &Buffer, pos: usize) -> Option<(usize, usize)> {
        let pat_len = self.pattern.len();
        if pat_len > 0 && pos + pat_len <= buffer.size() {
            let stop_pos = buffer.size() - pat_len;
            let mut pos = pos;
            while pos <= stop_pos {
                // Pattern matching occurs right-to-left.
                let mut i = pat_len;
                while i > 0 && self.pattern[i - 1] == self.buf_at(&buffer, pos + i - 1) {
                    i -= 1;
                }
                if i == 0 {
                    // Pattern matched.
                    return Some((pos, pos + pat_len));
                } else {
                    // Pattern match failed, so move position forward based on shift
                    // table and continue searching.
                    i -= 1;
                    pos += self
                        .shift
                        .get(&self.buf_at(buffer, pos + i))
                        .map(|n| if *n > i { 1 } else { i - n })
                        .unwrap_or(i + 1);
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
}

/// A regex-oriented pattern-matching algorithm.
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

        self.regex.find_at(buffer, pos_offset).and_then(|m| {
            // Convert starting and ending offsets into their respective character
            // positions.
            let Range { start, end } = m.range();
            let start_pos = etc::offset_to_pos(buffer, start);

            // This trick saves us from rescanning entire buffer to find ending offset.
            let end_pos = start_pos + etc::offset_to_pos(&buffer[start..], end - start);
            Some((start_pos, end_pos))
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
        self.search(&buf, pos)
            .or_else(|| if pos > 0 { self.search(&buf, 0) } else { None })
    }
}
