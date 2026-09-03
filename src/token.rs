//! Tokenization for syntax coloring.

use crate::buffer::Buffer;
use crate::etc;
use crate::syntax::Syntax;
use std::cell::RefCell;
use std::cmp;
use std::ops::{ControlFlow, Range};
use std::rc::Rc;

/// A means of tokenizing the contents of a [`Buffer`].
pub struct Tokenizer {
    /// The syntax configuration that drives tokenization.
    syntax: Syntax,

    /// The number of characters tokenized.
    chars: usize,

    /// The list of token spans generated during tokenization.
    spans: Vec<Span>,
}

pub type TokenizerRef = Rc<RefCell<Tokenizer>>;

/// A representation of the position in the [`Buffer`] that was used during tokenization,
/// as well as the corresponding token information.
#[derive(Copy, Clone, Debug)]
pub struct Position {
    /// The buffer position.
    pos: usize,

    /// The token corresponding to [`pos`](Self::pos).
    token: Token,

    /// The foreground color associated with this token or `None` if the token
    /// represents a gap.
    color: Option<u8>,
}

/// A token is essentially a [`Span`] that is decorated with the starting and ending
/// positions in the [`Buffer`] that was used during tokenization.
#[derive(Copy, Clone, Debug)]
struct Token {
    /// An index into [`Tokenizer::spans`].
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

impl Position {
    /// Returns the corresponding foreground color at this position or `None` if the
    /// position is contained inside a gap.
    #[inline(always)]
    pub fn color(&self) -> Option<u8> {
        self.color
    }
}

impl Tokenizer {
    /// Creates a new tokenizer using the `syntax` configuration.
    pub fn new(syntax: Syntax) -> Tokenizer {
        Tokenizer {
            syntax,
            chars: 0,
            spans: Vec::new(),
        }
    }

    /// Turns the tokenizer into a [`TokenizerRef`].
    pub fn into_ref(self) -> TokenizerRef {
        Rc::new(RefCell::new(self))
    }

    /// Returns a reference to the syntax configuration.
    pub fn syntax(&self) -> &Syntax {
        &self.syntax
    }

    /// Tokenizes `buffer` and returns a position at `0`.
    pub fn tokenize(&mut self, buffer: &Buffer) -> Position {
        self.spans.clear();
        self.chars = buffer.size();
        if self.chars > 0 {
            // Converting entire buffer to string is an unfortunate requirement since
            // regex library provide iterator support.
            let buf = buffer.iter().collect::<String>();

            // Keep track of byte offset and character position following last span.
            let mut offset = 0;
            let mut pos = 0;

            for cap in self.syntax.re.captures_iter(&buf) {
                // Get token information associated with capture group.
                let (id, Range { start, end }) = self.syntax.lookup(&cap);

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

        // Return position at 0.
        Position {
            pos: 0,
            token: Token {
                index: 0,
                start_pos: 0,
                end_pos: self.spans[0].len,
            },
            color: self.color(0),
        }
    }

    /// Finds the position at `pos` relative to position `p`.
    pub fn find(&self, p: Position, pos: usize) -> Position {
        let pos = cmp::min(pos, self.chars);
        if p.token.contains(pos) {
            Position { pos, ..p }
        } else {
            let token = if pos < p.pos {
                self.find_backward(p.token, pos)
            } else {
                self.find_forward(p.token, pos)
            };
            let color = self.color(token.index);
            Position { pos, token, color }
        }
    }

    /// Finds the position that is `n` characters after position `p`.
    pub fn forward(&self, p: Position, n: usize) -> Position {
        let pos = p.pos + n;
        self.find(p, pos)
    }

    /// Finds the position that is `n` characters before position `p`.
    #[allow(dead_code, reason = "possible future use")]
    pub fn backward(&self, p: Position, n: usize) -> Position {
        let pos = p.pos.saturating_sub(n);
        self.find(p, pos)
    }

    /// Returns the token corresponding to `pos` relative to the `from` token.
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

    /// Returns the token corresponding to `pos` relative to the `from` token.
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

    /// Inserts `len` characters at the position of `p` by expanding the length
    /// of the underlying span, returning an updated position.
    pub fn insert(&mut self, p: Position, len: usize) -> Position {
        if len > 0 {
            let token = &p.token;
            self.spans[token.index].len += len;
            self.chars += len;

            Position {
                token: Token {
                    end_pos: token.end_pos + len,
                    ..*token
                },
                ..p
            }
        } else {
            p
        }
    }

    /// Removes possibly many spans of `len` characters at the position of `p`,
    /// returning an updated position.
    pub fn remove(&mut self, p: Position, len: usize) -> Position {
        if len > 0 {
            // Find position following removal of specified length, noting that actual
            // length may be less if number of characters would extend beyond end.
            let end_p = self.find(p, p.pos + len);
            let len = end_p.pos - p.pos;
            let token = &p.token;
            let end_token = &end_p.token;

            let token = if token.index == end_token.index {
                // Removal is confined to current token, so simply reduce length of
                // existing span.
                self.spans[token.index].len -= len;
                Token {
                    end_pos: token.end_pos - len,
                    ..*token
                }
            } else {
                // Removal includes at least one span but possibly many. Evaluate
                // starting and ending boundaries to trim and/or include their
                // corresponding spans for removal.
                let start_index = if p.pos > token.start_pos {
                    self.spans[token.index].len = p.pos - token.start_pos;
                    token.index + 1
                } else {
                    token.index
                };

                let end_index = if end_p.pos < end_token.end_pos {
                    self.spans[end_token.index].len = end_token.end_pos - end_p.pos;
                    end_token.index - 1
                } else {
                    end_token.index
                };

                // Possibility exists for start index to be greater than end index under
                // sole condition: when starting and ending positions exist in adjacent
                // spans, so make sure this check is done to avoid panic!
                if start_index <= end_index {
                    self.spans.drain(start_index..=end_index);

                    // At least one span must always exist.
                    if self.spans.len() == 0 {
                        self.spans.push(Span::gap(0));
                    }
                }

                if start_index < self.spans.len() {
                    // Because start token is either truncated or entirely removed,
                    // start position of next token is always current position.
                    Token {
                        index: start_index,
                        start_pos: p.pos,
                        end_pos: p.pos + self.spans[start_index].len,
                    }
                } else {
                    // All spans following start index were removed, so token
                    // effectively points to prior span where its end position is
                    // current position.
                    Token {
                        index: start_index - 1,
                        start_pos: p.pos - self.spans[start_index - 1].len,
                        end_pos: p.pos,
                    }
                }
            };
            self.chars -= len;

            Position {
                pos: p.pos,
                token,
                color: self.color(token.index),
            }
        } else {
            p
        }
    }

    /// Returns the foreground color associated with the span at `index` or `None` if
    /// the span is a gap.
    fn color(&self, index: usize) -> Option<u8> {
        let Span { id, len: _ } = self.spans[index];
        self.syntax.color(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::tests::{build_empty_syntax, build_syntax};

    const TOKENS: [(&str, u8); 3] = [
        (r#"-?\d+(?:\.\d+)?(?:[eE]-?\d+)?"#, 1),
        (r#""(?:[^"\\]|(?:\\.))*""#, 2),
        (r#"\b(?:foo|bar)\b"#, 3),
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
    fn tokenize_buffer_no_tokens() {
        let mut tz = Tokenizer::new(build_empty_syntax());
        let buf = build_buffer();
        tz.tokenize(&buf);

        assert_eq!(tz.spans.len(), 1);
        assert_eq!(tz.spans[0].id, 0);
        assert_eq!(tz.spans[0].len, buf.size());
    }

    #[test]
    fn find_position() {
        // Pairs of (pos, index) associations.
        const POS_TOKENS: [(usize, usize); 5] = [(3, 0), (19, 4), (11, 3), (40, 11), (29, 7)];

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let mut token_pos = tz.tokenize(&buf);

        for p in POS_TOKENS {
            token_pos = tz.find(token_pos, p.0);

            // Verify that (pos, index) values match.
            assert_eq!(token_pos.pos, p.0);
            assert_eq!(token_pos.token.index, p.1);

            // Verify that token information matches what exists in spans.
            let (id, len, _) = SPANS[p.1];
            assert!(token_pos.token.start_pos <= p.0);
            assert!(token_pos.token.end_pos > p.0);
            assert_eq!(token_pos.token.end_pos - token_pos.token.start_pos, len);
            assert_eq!(token_pos.color, color_of(id));
        }
    }

    #[test]
    fn position_forward() {
        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let mut token_pos = tz.tokenize(&buf);

        while token_pos.pos < tz.chars {
            // Verify that token information matches what exists in spans.
            let (id, len, _) = SPANS[token_pos.token.index];
            assert!(token_pos.token.start_pos <= token_pos.pos);
            assert!(token_pos.token.end_pos > token_pos.pos);
            assert_eq!(token_pos.token.end_pos - token_pos.token.start_pos, len);
            assert_eq!(token_pos.color, color_of(id));
            token_pos = tz.forward(token_pos, 1);
        }
    }

    #[test]
    fn position_backward() {
        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let mut token_pos = tz.tokenize(&buf);

        while token_pos.pos > 0 {
            // Verify that token information matches what exists in spans.
            let (id, len, _) = SPANS[token_pos.token.index];
            assert!(token_pos.token.start_pos <= token_pos.pos);

            // Special edge case when pos is at end of buffer.
            if token_pos.pos < tz.chars {
                assert!(token_pos.token.end_pos > token_pos.pos);
            } else {
                assert!(token_pos.token.end_pos == token_pos.pos);
            }

            assert_eq!(token_pos.token.end_pos - token_pos.token.start_pos, len);
            assert_eq!(token_pos.color, color_of(id));
            token_pos = tz.backward(token_pos, 1);
        }
    }

    #[test]
    fn insert_start_of_span() {
        const POS: usize = 24;
        const LEN: usize = 7;

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let token_pos = tz.tokenize(&buf);
        let chars = tz.chars;

        let token_pos = tz.find(token_pos, POS);
        let (id, len, _) = SPANS[token_pos.token.index];
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, POS);

        // Expands span referenced by token_pos token.
        let token_pos = tz.insert(token_pos, LEN);
        assert_eq!(tz.chars, chars + LEN);
        assert_eq!(tz.spans.len(), SPANS.len());

        // Verify that token at current position is expanded span.
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, POS);
        assert_eq!(token_pos.token.end_pos, POS + len + LEN);
        assert_eq!(token_pos.color, color_of(id));
    }

    #[test]
    fn insert_middle_of_span() {
        const POS: usize = 26;
        const LEN: usize = 7;
        const START_POS: usize = 24;

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let token_pos = tz.tokenize(&buf);
        let chars = tz.chars;

        let token_pos = tz.find(token_pos, POS);
        let (id, len, _) = SPANS[token_pos.token.index];
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, START_POS);

        // Expands span referenced by token_pos token.
        let token_pos = tz.insert(token_pos, LEN);
        assert_eq!(tz.chars, chars + LEN);
        assert_eq!(tz.spans.len(), SPANS.len());

        // Verify that token at current position is newly inserted span.
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, START_POS);
        assert_eq!(token_pos.token.end_pos, START_POS + len + LEN);
        assert_eq!(token_pos.color, color_of(id));
    }

    #[test]
    fn remove_single_span() {
        const POS: usize = 27;
        const LEN: usize = 3;
        const START_POS: usize = 24;

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let token_pos = tz.tokenize(&buf);
        let chars = tz.chars;

        let token_pos = tz.find(token_pos, POS);
        let (id, len, _) = SPANS[token_pos.token.index];
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, START_POS);

        // Results in zero spans being removed.
        let token_pos = tz.remove(token_pos, LEN);
        assert_eq!(tz.chars, chars - LEN);
        assert_eq!(tz.spans.len(), SPANS.len());

        // Verify that current token only changed in length.
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, START_POS);
        assert_eq!(token_pos.token.end_pos, START_POS + (len - LEN));
        assert_eq!(token_pos.color, color_of(id));
    }

    #[test]
    fn remove_single_span_entire() {
        const POS: usize = 24;
        const LEN: usize = 7;

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let token_pos = tz.tokenize(&buf);
        let chars = tz.chars;

        let token_pos = tz.find(token_pos, POS);
        let (id, len, _) = SPANS[token_pos.token.index + 1];
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, POS);

        // Results in current span being removed.
        let token_pos = tz.remove(token_pos, LEN);
        assert_eq!(tz.chars, chars - LEN);
        assert_eq!(tz.spans.len(), SPANS.len() - 1);

        // Verify that new token at matches following token.
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, POS);
        assert_eq!(token_pos.token.end_pos, POS + len);
        assert_eq!(token_pos.color, color_of(id));
    }

    #[test]
    fn remove_multiple_spans_inclusive() {
        const POS: usize = 6;
        const LEN: usize = 26;

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let token_pos = tz.tokenize(&buf);
        let chars = tz.chars;

        let token_pos = tz.find(token_pos, POS);
        let (id, len, _) = SPANS[token_pos.token.index + 8];
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, POS);

        // Results in mutiple spans being removed, including edges.
        let token_pos = tz.remove(token_pos, LEN);
        assert_eq!(tz.chars, chars - LEN);
        assert_eq!(tz.spans.len(), SPANS.len() - 8);

        // Verify that new token matches token following last span removed.
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, POS);
        assert_eq!(token_pos.token.end_pos, POS + len);
        assert_eq!(token_pos.color, color_of(id));
    }

    #[test]
    fn remove_multiple_spans_exclusive() {
        const POS: usize = 7;
        const LEN: usize = 23;
        const START_POS: usize = 6;

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let token_pos = tz.tokenize(&buf);
        let chars = tz.chars;

        // Find last token whose prefix will be truncated.
        let token_pos = tz.find(token_pos, POS + LEN);
        let (id, _, _) = SPANS[token_pos.token.index];
        let len = token_pos.token.end_pos - token_pos.pos;

        let token_pos = tz.find(token_pos, POS);
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, START_POS);

        // Results in mutiple spans being removed, excluding edges.
        let token_pos = tz.remove(token_pos, LEN);
        assert_eq!(tz.chars, chars - LEN);
        assert_eq!(tz.spans.len(), SPANS.len() - 5);

        // Verify that new token matches final token whose prefix was truncated.
        assert_eq!(token_pos.pos, POS);
        assert_eq!(token_pos.token.start_pos, POS);
        assert_eq!(token_pos.token.end_pos, POS + len);
        assert_eq!(token_pos.color, color_of(id));
    }

    #[test]
    fn remove_spans_to_end() {
        const POS: usize = 7;

        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let token_pos = tz.tokenize(&buf);
        let chars = tz.chars;

        let token_pos = tz.find(token_pos, POS);
        let token_pos = tz.remove(token_pos, chars - POS);
        assert_eq!(tz.chars, POS);
        assert_eq!(token_pos.pos, POS);
    }

    #[test]
    fn remove_all_spans() {
        let mut tz = build_tokenizer();
        let buf = build_buffer();
        let token_pos = tz.tokenize(&buf);

        let token_pos = tz.remove(token_pos, tz.chars);
        assert_eq!(tz.chars, 0);
        assert_eq!(token_pos.pos, 0);
    }

    fn build_tokenizer() -> Tokenizer {
        Tokenizer::new(build_syntax())
    }

    fn build_buffer() -> Buffer {
        let mut buf = Buffer::new();
        buf.insert_str(TEXT);
        buf.set_pos(0);
        buf
    }

    fn color_of(id: usize) -> Option<u8> {
        if id > 0 { Some(TOKENS[id - 1].1) } else { None }
    }
}
