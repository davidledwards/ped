//! A collection of types and functions for navigating [`Buffer`]s.

use crate::buffer::Buffer;
use crate::error::{Error, Result};
use std::fmt::{self, Display, Formatter};
use std::ops::ControlFlow;

/// Represents a logical position in a [`Buffer`] denoted by _line_ and _column_,
/// both of which are `0`-based.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Location {
    pub line: u32,
    pub col: u32,
}

impl Location {
    /// A location of (`0`, `0`).
    pub const TOP: Location = Location::new(0, 0);

    /// Creates a new location with `line` and `col`.
    pub const fn new(line: u32, col: u32) -> Location {
        Location { line, col }
    }

    /// Creates a new location with the assumption that `line` and `col` are `1`-based
    /// values, thus both are _safely_ decremented by `1` since internal representations
    /// of location are `0`-based.
    fn from_user(line: u32, col: u32) -> Location {
        Location {
            line: line.saturating_sub(1),
            col: col.saturating_sub(1),
        }
    }

    /// Parses the location in `value` and returns the corresponding location or an
    /// error if the parse failed.
    ///
    /// A location string must conform to the following syntax:
    ///
    /// _line_ \[ (`,` | `:`) _col_ \]
    ///
    /// The values of _line_ and _col_ in `value` are presumed to be `1`-based, thus
    /// both are _safely_ decremented by `1` since internal representations of location
    /// are `0`-based.
    pub fn parse(value: &str) -> Result<Location> {
        let vs = value
            .split([',', ':'])
            .map(|v| v.trim())
            .filter(|v| v.len() > 0)
            .collect::<Vec<_>>();

        match &vs[..] {
            [line] => match line.parse::<u32>() {
                Ok(l) => Ok(Location::from_user(l, 1)),
                Err(e) => Err(Error::parse_location(line, &e)),
            },
            [line, col] => match (line.parse::<u32>(), col.parse::<u32>()) {
                (Ok(l), Ok(c)) => Ok(Location::from_user(l, c)),
                (Err(e), _) => Err(Error::parse_location(line, &e)),
                (_, Err(e)) => Err(Error::parse_location(col, &e)),
            },
            _ => Err(Error::general(&format!("{value}: too many arguments"))),
        }
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.line, self.col)
    }
}

/// Returns the location corresponding to `pos` in `buf`.
///
/// If `pos` is larger than the size of `buf`, then the resulting location is anchored
/// to [`buf.size()`](Buffer::size).
pub fn find_location(buf: &Buffer, pos: usize) -> Location {
    let (line, col) = buf.forward(0).take(pos).fold((0, 0), |(line, col), c| {
        if c == '\n' {
            (line + 1, 0)
        } else {
            (line, col + 1)
        }
    });
    Location::new(line, col)
}

/// Returns the position in `buf` of the given location `loc`.
///
/// If `loc.col` would extend beyond the end of `loc.line`, or more precisely, if the
/// distance between the starting position of `loc.line` and the next `\n` is less
/// than `loc.col`, then the resulting position will be anchored to the `\n`.
///
/// If `loc.line` would extend beyond the end of the buffer, then the end of buffer
/// is returned.
pub fn find_pos(buf: &Buffer, loc: Location) -> usize {
    // Find starting position of line.
    let pos = if loc.line > 0 {
        let r = buf.forward(0).index().try_fold(0, |l, (pos, c)| {
            if c == '\n' {
                let l = l + 1;
                if l == loc.line {
                    ControlFlow::Break(pos + 1)
                } else {
                    ControlFlow::Continue(l)
                }
            } else {
                ControlFlow::Continue(l)
            }
        });
        match r {
            ControlFlow::Break(pos) => pos,
            _ => buf.size(),
        }
    } else {
        0
    };

    // Find position of column relative to line.
    if pos < buf.size() && loc.col > 0 {
        let r = buf.forward(pos).index().try_fold(0, |n, (pos, c)| {
            if n == loc.col || c == '\n' {
                ControlFlow::Break(pos)
            } else {
                ControlFlow::Continue(n + 1)
            }
        });
        match r {
            ControlFlow::Break(pos) => pos,
            _ => buf.size(),
        }
    } else {
        pos
    }
}

/// Returns the position of the first character of the line relative to `pos`.
///
/// Specifically, this function returns the position of the character following the
/// first `\n` encountered when scanning backwards from `pos`, or returns `0` if the
/// beginning of buffer is reached.
///
/// Note that when scanning backwards, `pos` is an _exclusive_ bound.
pub fn find_start_line(buf: &Buffer, pos: usize) -> usize {
    buf.backward(pos)
        .index()
        .find(|&(_, c)| c == '\n')
        .map(|(_pos, _)| _pos + 1)
        .unwrap_or(0)
}

/// Returns a tuple containing the position of the next line relative to `pos` and
/// a boolean indicating if the end of buffer has been reached.
///
/// Specifically, this function returns the position following the first `\n`
/// encountered when scanning forward from `pos`, or returns the end of buffer
/// position if reached first. The end-of-buufer scenario is the only condition which
/// would cause the second tuple value to return `true`.
///
/// Note that when scanning forward, `pos` is an _inclusive_ bound.
pub fn find_next_line(buf: &Buffer, pos: usize) -> (usize, bool) {
    buf.forward(pos)
        .index()
        .find(|&(_, c)| c == '\n')
        .map(|(_pos, _)| (_pos + 1, false))
        .unwrap_or((buf.size(), true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_location_of_pos() {
        const TEXT: &str = "Lorem\nipsum\ndolor\nsit\namet,\nconsectetur\nporttitor";

        let mut buf = Buffer::new();
        buf.insert_str(TEXT);

        // Beginning of buffer is always (0, 0).
        let loc = find_location(&buf, 0);
        assert_eq!(loc.line, 0);
        assert_eq!(loc.col, 0);

        // Check EOL boundary.
        let loc = find_location(&buf, 11);
        assert_eq!(loc.line, 1);
        assert_eq!(loc.col, 5);
        let loc = find_location(&buf, 12);
        assert_eq!(loc.line, 2);
        assert_eq!(loc.col, 0);

        // Check somewhere in middle of buffer.
        let loc = find_location(&buf, 14);
        assert_eq!(loc.line, 2);
        assert_eq!(loc.col, 2);

        // Positions beyond end of buffer are bounded.
        let loc = find_location(&buf, usize::MAX);
        assert_eq!(loc.line, 6);
        assert_eq!(loc.col, 9);
    }

    #[test]
    fn find_pos_of_location() {
        const TEXT: &str = "Lorem\nipsum\ndolor\nsit\namet,\nconsectetur\nporttitor";

        let mut buf = Buffer::new();
        buf.insert_str(TEXT);

        // Check normal and edge cases from beginning of buffer.
        let pos = find_pos(&buf, Location::new(0, 3));
        assert_eq!(pos, 3);
        let pos = find_pos(&buf, Location::new(0, 10));
        assert_eq!(pos, 5);

        // Check normal and edge cases from middle of buffer.
        let pos = find_pos(&buf, Location::new(2, 2));
        assert_eq!(pos, 14);
        let pos = find_pos(&buf, Location::new(2, 10));
        assert_eq!(pos, 17);

        // Check normal and edge cases near end of buffer.
        let pos = find_pos(&buf, Location::new(6, 5));
        assert_eq!(pos, 45);
        let pos = find_pos(&buf, Location::new(6, 10));
        assert_eq!(pos, buf.size());
    }

    #[test]
    fn find_start_line_pos() {
        const TEXT: &str = "abc\ndef\nghi";

        let mut buf = Buffer::new();
        buf.insert_str(TEXT);

        // All chars in `def\n` range should find the same beginning of line.
        for pos in 4..8 {
            let p = find_start_line(&buf, pos);
            assert_eq!(p, 4);
        }

        // All chars in `abc\n` range should find the same beginning of line, which
        // also happens to be beginning of buffer.
        for pos in 0..4 {
            let p = find_start_line(&buf, pos);
            assert_eq!(p, 0);
        }
    }

    #[test]
    fn find_next_line_pos() {
        const TEXT: &str = "abc\ndef\nghi";

        let mut buf = Buffer::new();
        buf.insert_str(TEXT);

        // All chars in `def\n` range should find the same next line.
        for pos in 4..8 {
            let (p, eob) = find_next_line(&buf, pos);
            assert_eq!(p, 8);
            assert!(!eob);
        }

        // All chars in `ghi` range should yield the end of buffer position.
        for pos in 8..11 {
            let (p, eob) = find_next_line(&buf, pos);
            assert_eq!(p, 11);
            assert!(eob);
        }
    }
}
