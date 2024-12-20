//! Syntax highlighting.

use crate::buffer::Buffer;
use crate::color::Color;
use crate::error::{Error, Result};
use crate::etc;
use regex_lite::{Captures, Match, Regex, RegexBuilder};
use std::cmp;
use std::ops::{ControlFlow, Range};

struct Span {
    id: usize,
    len: usize,
}

impl Span {
    fn new(id: usize, len: usize) -> Span {
        Span { id, len }
    }

    fn gap(len: usize) -> Span {
        Span { id: 0, len }
    }
}

struct Def {
    id: usize, // 0 == gap
    name: String,
    pattern: String,
    color: Color,
}

pub struct Tokenizer {
    re: Regex,
    chars: usize,
    defs: Vec<Def>,
    spans: Vec<Span>,
}

#[derive(Debug)]
struct Token {
    index: usize,
    start_pos: usize,
    end_pos: usize,
}

impl Token {
    #[inline(always)]
    fn contains(&self, pos: usize) -> bool {
        pos >= self.start_pos && pos < self.end_pos
    }
}

pub struct Cursor<'a> {
    tokenizer: &'a Tokenizer,

    // pos of cursor in buffer
    pos: usize,

    // token corresponding to pos
    token: Token,

    color: Color,
}

impl<'a> Cursor<'a> {
    pub fn color(&self) -> Color {
        self.color
    }

    pub fn forward(self, n: usize) -> Cursor<'a> {
        let pos = self.pos + n;
        self.find(pos)
    }

    pub fn backward(self, n: usize) -> Cursor<'a> {
        let pos = self.pos.saturating_sub(n);
        self.find(pos)
    }

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
            let color = self.tokenizer.span_color(token.index);
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
    pub fn new(tokens: Vec<(String, Color)>) -> Result<Tokenizer> {
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

    pub fn find(&self, pos: usize) -> Cursor<'_> {
        let pos = cmp::min(pos, self.chars);

        let start_token = Token {
            index: 0,
            start_pos: 0,
            end_pos: self.spans[0].len,
        };

        let token = self.find_forward(start_token, pos);
        let color = self.span_color(token.index);

        Cursor {
            tokenizer: &self,
            pos,
            token,
            color,
        }
    }

    fn span_color(&self, index: usize) -> Color {
        let Span { id, len: _ } = self.spans[index];
        if id == 0 {
            // todo
            // - this should be a value provided during initialization
            Color::ZERO
        } else {
            self.defs[id - 1].color
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

    pub fn tokenize(&mut self, buffer: &Buffer) {
        // scan entire buffer and rebuild token list
        self.chars = buffer.size();
        self.spans.clear();

        if self.chars > 0 {
            // convert buffer to a string before scanning
            // this is terribly inefficient, but regex library has no means of using an
            // iterator, likely because of the need to back up when match paths fail
            let buf = buffer.iter().collect::<String>();

            // keep the pos in buffer following the last token in token_spans
            let mut offset = 0;
            let mut pos = 0;

            for cap in self.re.captures_iter(&buf) {
                // find token based on capture group, must be _n where n is the index
                // into self.token_defs
                //
                // add gap span if there is non-zero distance between prior token and
                // this token
                //
                // byte offsets produced by regex library need to be converted pos-th
                // character in the buffer
                let (id, m) = self.lookup_token(&cap);
                let Range { start, end } = m.range();

                let start_pos = pos + etc::offset_to_pos(&buf[offset..], start - offset);
                let end_pos = start_pos + etc::offset_to_pos(&buf[start..], end - start);

                if start_pos > pos {
                    self.spans.push(Span::gap(start_pos - pos));
                }
                self.spans.push(Span::new(id, end_pos - start_pos));
                offset = end;
                pos = end_pos;
            }

            // add gap to end if one exists
            if offset < buf.len() {
                let end_pos = pos + etc::offset_to_pos(&buf[offset..], buf.len() - offset);
                self.spans.push(Span::gap(end_pos - pos));
            }
        } else {
            // add zero-length gap to make other functions work correctly
            self.spans.push(Span::gap(0));
        }
    }

    fn lookup_token<'a>(&self, cap: &'a Captures) -> (usize, Match<'a>) {
        self.defs
            .iter()
            .find_map(|def| cap.name(&def.name).map(|m| (def.id, m)))
            .unwrap_or_else(|| panic!("{}: capture group expected for token", &cap[0]))
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
                    Color::ZERO
                } else {
                    TOKENS[id - 1].1
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
                    Color::ZERO
                } else {
                    TOKENS[id - 1].1
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
                    Color::ZERO
                } else {
                    TOKENS[id - 1].1
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
