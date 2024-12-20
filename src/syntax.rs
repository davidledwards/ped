//! Syntax coloring.

use crate::buffer::Buffer;
use crate::color::Color;
use crate::error::{Error, Result};
use crate::etc;
use regex_lite::{Captures, Regex, RegexBuilder};
use std::cmp;
use std::ops::{ControlFlow, Range};

/// A means of tokenizing the contents of a [`Buffer`].
pub struct Tokenizer {
    /// A single regular expression aggregating all token definitions, each adorned
    /// with its own capture group name.
    re: Regex,

    /// A collection of token definitions whose order is crucial, since indexes are
    /// used during tokenization to refer to these definitions.
    defs: Vec<Def>,

    /// The number of characters tokenized.
    chars: usize,

    /// The list of token spans generated during tokenization.
    spans: Vec<Span>,
}

/// A cursor represents a position in the [`Buffer`] used during tokenization as well
/// as the applicable token information.
pub struct Cursor<'a> {
    /// A reference to the tokenizer that produced this cursor.
    tokenizer: &'a Tokenizer,

    /// The buffer position associated with this cursor.
    pos: usize,

    /// The applicable token corresponding to [`pos`](Self::pos).
    token: Token,

    /// The color associated with this token or `None` if the token represents a gap.
    color: Option<Color>,
}

/// A token definition represents a regular expression with a unique identifier that
/// is used in forming capture group names.
struct Def {
    /// A unique identifier representing this token.
    ///
    /// The value of `0` is reserved for special [spans](Span) that represents _gaps_
    /// between recognized tokens.
    id: usize,

    /// The unique capture group name assigned to this token, which is formed using
    /// [`id`](Self::id).
    name: String,

    /// The regular expression for this token.
    pattern: String,

    /// The color associated with this token.
    color: Color,
}

/// A token is essentially a [`Span`] that is decorated with the starting and ending
/// positions in the [`Buffer`] that was used during tokenization.
struct Token {
    /// An index into [`Tokenizer::defs`].
    index: usize,

    /// The starting position of the token, which is an _inclusive_ bound.
    start_pos: usize,

    /// The ending position of the token, which is an _exclusive_ bound.
    end_pos: usize,
}

/// A span represents a slice of text that matchs a token `id`.
struct Span {
    id: usize,
    len: usize,
}

impl Span {
    fn gap(len: usize) -> Span {
        Span { id: 0, len }
    }

    fn token(id: usize, len: usize) -> Span {
        Span { id, len }
    }
}

impl Token {
    #[inline(always)]
    fn contains(&self, pos: usize) -> bool {
        pos >= self.start_pos && pos < self.end_pos
    }
}

impl<'a> Cursor<'a> {
    /// Returns the applicable color at this cursor position or `None` if the cursor
    /// is contained inside a gap.
    pub fn color(&self) -> Option<Color> {
        self.color
    }

    /// Moves the cursor forward by `n` characters, though not to extend beyond
    /// [`Tokenizer::size`].
    pub fn forward(self, n: usize) -> Cursor<'a> {
        let pos = self.pos + n;
        self.find(pos)
    }

    /// Moves the cursor backward by `n` characters, though not to extend beyond `0`.
    pub fn backward(self, n: usize) -> Cursor<'a> {
        let pos = self.pos.saturating_sub(n);
        self.find(pos)
    }

    /// Returns the cursor at position `pos`, though not to extend beyond
    /// [`Tokenizer::size`].
    pub fn find(self, pos: usize) -> Cursor<'a> {
        let pos = cmp::min(pos, self.tokenizer.chars);
        if self.token.contains(pos) {
            Cursor { pos, ..self }
        } else {
            let token = if pos < self.pos {
                self.tokenizer.find_backward(self.token, pos)
            } else {
                self.tokenizer.find_forward(self.token, pos)
            };
            let color = self.tokenizer.color(token.index);
            Cursor {
                pos,
                token,
                color,
                ..self
            }
        }
    }
}

impl Tokenizer {
    /// Creates a new tokenizer using `tokens`, which are tuples containing a regular
    /// expression and a color.
    ///
    /// If any of the regular expressions are malformed or the aggregate size of all
    /// regular expressions is too large, then an error is returned.
    pub fn new(tokens: Vec<(String, Color)>) -> Result<Tokenizer> {
        // Tokens are adorned with capture group names of "_<id>" where <id> is the
        // index of the token definition offset by 1. Offset is required because
        // token id 0 is reserved for gap spans.
        let defs = tokens
            .iter()
            .enumerate()
            .map(|(i, (pattern, color))| Def {
                id: i + 1,
                name: format!("_{}", i + 1),
                pattern: pattern.clone(),
                color: *color,
            })
            .collect::<Vec<_>>();

        // Join all token regular expressions using capture group names.
        let pattern = defs
            .iter()
            .map(|def| format!("(?<{}>{})", def.name, def.pattern))
            .collect::<Vec<_>>()
            .join("|");

        let re = match RegexBuilder::new(&pattern).multi_line(true).build() {
            Ok(re) => re,
            Err(e) => return Err(Error::invalid_regex(&pattern, &e)),
        };

        let this = Tokenizer {
            re,
            chars: 0,
            defs,
            spans: Vec::new(),
        };
        Ok(this)
    }

    /// Tokenizes `buffer`.
    ///
    /// Even though lifetimes restrict the use of prior [cursors](Cursor), it is
    /// important to remember that any information extracted from prior cursors should
    /// be considered invalid.
    pub fn tokenize(&mut self, buffer: &Buffer) {
        self.spans.clear();
        self.chars = buffer.size();
        if self.chars > 0 {
            // Converting entire buffer to string is an unfortunate requirement since
            // regex library provide iterator support.
            let buf = buffer.iter().collect::<String>();

            // Keep track of byte offset and character position following last span.
            let mut offset = 0;
            let mut pos = 0;

            for cap in self.re.captures_iter(&buf) {
                // Get token information associated with capture group.
                let (id, Range { start, end }) = self.lookup_token(&cap);

                // Byte offsets returned by regex library must be converted to their
                // corresponding character positions.
                let start_pos = pos + etc::offset_to_pos(&buf[offset..], start - offset);
                let end_pos = start_pos + etc::offset_to_pos(&buf[start..], end - start);

                // Insert gap span if non-zero distance exists between this token and
                // prior token.
                if start_pos > pos {
                    self.spans.push(Span::gap(start_pos - pos));
                }

                // Add new token span.
                self.spans.push(Span::token(id, end_pos - start_pos));
                offset = end;
                pos = end_pos;
            }

            // Add gap span if non-zero distance between last token and end of buffer.
            if offset < buf.len() {
                let end_pos = pos + etc::offset_to_pos(&buf[offset..], buf.len() - offset);
                self.spans.push(Span::gap(end_pos - pos));
            }
        } else {
            // An empty buffer requires zero-length gap to be appended to spans to
            // ensure other functions work correctly.
            self.spans.push(Span::gap(0));
        }
    }

    /// Returns the cursor at position `pos`, though not to extend beyond
    /// [`Tokenizer::size`].
    pub fn find(&self, pos: usize) -> Cursor<'_> {
        let pos = cmp::min(pos, self.chars);
        let token = self.find_forward(
            Token {
                index: 0,
                start_pos: 0,
                end_pos: self.spans[0].len,
            },
            pos,
        );
        let color = self.color(token.index);
        Cursor {
            tokenizer: &self,
            pos,
            token,
            color,
        }
    }

    /// Returns the token applicable to `pos` relative to the `from` token.
    ///
    /// If `pos` does occur _after_ `from`, then this function will panic.
    fn find_forward(&self, from: Token, pos: usize) -> Token {
        debug_assert!(pos >= from.start_pos);
        let result =
            self.spans
                .iter()
                .skip(from.index + 1)
                .try_fold(from, |token, Span { id: _, len }| {
                    if pos >= token.end_pos {
                        ControlFlow::Continue(Token {
                            index: token.index + 1,
                            start_pos: token.end_pos,
                            end_pos: token.end_pos + len,
                        })
                    } else {
                        ControlFlow::Break(token)
                    }
                });
        match result {
            ControlFlow::Break(token) => token,
            ControlFlow::Continue(token) => token,
        }
    }

    /// Returns the token applicable to `pos` relative to the `from` token.
    ///
    /// If `pos` does occur _before_ `from`, then this function will panic.
    fn find_backward(&self, from: Token, pos: usize) -> Token {
        debug_assert!(pos <= from.start_pos);
        let result = self.spans.iter().take(from.index).rev().try_fold(
            from,
            |token, Span { id: _, len }| {
                if pos < token.start_pos {
                    ControlFlow::Continue(Token {
                        index: token.index - 1,
                        start_pos: token.start_pos - len,
                        end_pos: token.start_pos,
                    })
                } else {
                    ControlFlow::Break(token)
                }
            },
        );
        match result {
            ControlFlow::Break(token) => token,
            ControlFlow::Continue(token) => token,
        }
    }

    pub fn dump(&self, buffer: &Buffer) {
        let mut pos = 0;

        for (i, span) in self.spans.iter().enumerate() {
            let Span { id, len } = span;
            let text = if *id == 0 {
                String::from("")
            } else {
                buffer.copy(pos, pos + len).iter().collect::<String>()
            };
            eprintln!("[{i}]: <token {id}>: pos={pos}, len={len}, text={text}");
            pos += len;
        }
    }

    /// Returns the token id and the byte offset range for the matching capture group
    /// `cap`.
    ///
    /// This function panics if the capture group does not match any of the expected
    /// names, as such a condition would indicate a correctness problem.
    fn lookup_token(&self, cap: &Captures) -> (usize, Range<usize>) {
        self.defs
            .iter()
            .find_map(|def| cap.name(&def.name).map(|m| (def.id, m.range())))
            .unwrap_or_else(|| panic!("{}: capture group expected for token", &cap[0]))
    }

    /// Returns the color associated with the span at `index` or `None` if the span
    /// is a gap.
    fn color(&self, index: usize) -> Option<Color> {
        let Span { id, len: _ } = self.spans[index];
        if id > 0 {
            Some(self.defs[id - 1].color)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKENS: [(&str, Color); 3] = [
        (r#"-?\d+(?:\.\d+)?(?:[eE]-?\d+)?"#, Color::new(1, 1)),
        (r#""(?:[^"\\]|(?:\\.))*""#, Color::new(2, 2)),
        (r#"\b(?:foo|bar)\b"#, Color::new(3, 3)),
    ];

    const TEXT: &str = "Lorem 1.2\n34 ip😀sum foo \"dolor\" bar -9.87e-6\n";

    const SPANS: [(usize, usize, &str); 13] = [
        (0, 6, "Lorem "),
        (1, 3, "1.2"),
        (0, 1, "\n"),
        (1, 2, "34"),
        (0, 8, " ip😀sum "),
        (3, 3, "foo"),
        (0, 1, " "),
        (2, 7, "\"dolor\""),
        (0, 1, " "),
        (3, 3, "bar"),
        (0, 1, " "),
        (1, 8, "-9.87e-6"),
        (0, 1, "\n"),
    ];

    #[test]
    fn new_tokenizer() {
        let tz = build_tokenizer();
        assert_eq!(tz.chars, 0);
        assert_eq!(tz.spans.len(), 0);
        assert_eq!(tz.defs.len(), TOKENS.len());
        for (i, def) in tz.defs.iter().enumerate() {
            assert_eq!(def.id, i + 1);
            assert_eq!(def.name, format!("_{}", i + 1));
            assert_eq!(def.pattern, TOKENS[i].0);
            assert_eq!(def.color, TOKENS[i].1);
        }
    }

    #[test]
    fn empty_tokenizer() {
        let tz = Tokenizer::new(Vec::new()).unwrap();
        assert_eq!(tz.chars, 0);
        assert_eq!(tz.spans.len(), 0);
        assert_eq!(tz.defs.len(), 0);
    }

    #[test]
    fn invalid_token() {
        let tokens = vec![("(bad".to_string(), Color::ZERO)];
        let tz = Tokenizer::new(tokens);
        assert!(tz.is_err());
    }

    #[test]
    fn tokenize_buffer() {
        let mut tz = build_tokenizer();
        let buf = build_buffer();
        tz.tokenize(&buf);
        assert_eq!(tz.spans.len(), SPANS.len());

        let mut pos = 0;
        for (i, span) in tz.spans.iter().enumerate() {
            assert_eq!(span.id, SPANS[i].0);
            assert_eq!(span.len, SPANS[i].1);
            assert_eq!(buf.copy_as_string(pos, pos + span.len), SPANS[i].2);
            pos += span.len;
        }
    }

    #[test]
    fn tokenize_empty_buffer() {
        let mut tz = build_tokenizer();
        let buf = Buffer::new();
        tz.tokenize(&buf);

        assert_eq!(tz.spans.len(), 1);
        assert_eq!(tz.spans[0].id, 0);
        assert_eq!(tz.spans[0].len, 0);
    }

    #[test]
    fn find_cursor() {
        // Pairs of (pos, index) associations.
        const POS_TOKENS: [(usize, usize); 5] = [(3, 0), (19, 4), (11, 3), (40, 11), (29, 7)];

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        tz.tokenize(&buf);

        let mut cursor = tz.find(0);
        for p in POS_TOKENS {
            cursor = cursor.find(p.0);

            // Verify that (pos, index) values match.
            assert_eq!(cursor.pos, p.0);
            assert_eq!(cursor.token.index, p.1);

            // Verify that token information matches what exists in spans.
            let (id, len, _) = SPANS[p.1];
            assert!(cursor.token.start_pos <= p.0);
            assert!(cursor.token.end_pos > p.0);
            assert_eq!(cursor.token.end_pos - cursor.token.start_pos, len);
            assert_eq!(
                cursor.color,
                if id == 0 {
                    None
                } else {
                    Some(TOKENS[id - 1].1)
                }
            );
        }
    }

    #[test]
    fn cursor_forward() {
        let mut tz = build_tokenizer();
        let buf = build_buffer();
        tz.tokenize(&buf);

        let mut cursor = tz.find(0);
        while cursor.pos < tz.chars {
            // Verify that token information matches what exists in spans.
            let (id, len, _) = SPANS[cursor.token.index];
            assert!(cursor.token.start_pos <= cursor.pos);
            assert!(cursor.token.end_pos > cursor.pos);
            assert_eq!(cursor.token.end_pos - cursor.token.start_pos, len);
            assert_eq!(
                cursor.color,
                if id == 0 {
                    None
                } else {
                    Some(TOKENS[id - 1].1)
                }
            );

            cursor = cursor.forward(1);
        }
    }

    #[test]
    fn cursor_backward() {
        let mut tz = build_tokenizer();
        let buf = build_buffer();
        tz.tokenize(&buf);

        let mut cursor = tz.find(tz.chars);
        while cursor.pos > 0 {
            // Verify that token information matches what exists in spans.
            let (id, len, _) = SPANS[cursor.token.index];
            assert!(cursor.token.start_pos <= cursor.pos);

            // Special edge case when pos is at end of buffer.
            if cursor.pos < tz.chars {
                assert!(cursor.token.end_pos > cursor.pos);
            } else {
                assert!(cursor.token.end_pos == cursor.pos);
            }

            assert_eq!(cursor.token.end_pos - cursor.token.start_pos, len);
            assert_eq!(
                cursor.color,
                if id == 0 {
                    None
                } else {
                    Some(TOKENS[id - 1].1)
                }
            );

            cursor = cursor.backward(1);
        }
    }

    fn build_tokenizer() -> Tokenizer {
        Tokenizer::new(build_tokens()).unwrap()
    }

    fn build_tokens() -> Vec<(String, Color)> {
        TOKENS
            .iter()
            .map(|(token, color)| (token.to_string(), *color))
            .collect()
    }

    fn build_buffer() -> Buffer {
        let mut buf = Buffer::new();
        buf.insert_str(TEXT);
        buf.set_pos(0);
        buf
    }
}
