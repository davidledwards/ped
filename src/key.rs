//! Keyboard reader.

use crate::error::Result;
use std::io::{self, Bytes, Read, Stdin};
use std::str::from_utf8;

/// The set of keys recognized by [`Keyboard`]s.
#[derive(Eq, PartialEq, Hash, Debug)]
pub enum Key {
    None,
    Control(u8),
    Char(char),
    Delete,
    Insert,
    ShiftTab,
    Up(Shift, Ctrl),
    Down(Shift, Ctrl),
    Right(Shift, Ctrl),
    Left(Shift, Ctrl),
    Home(Shift, Ctrl),
    End(Shift, Ctrl),
    PageUp(Shift, Ctrl),
    PageDown(Shift, Ctrl),
    Function(u8),
}

/// Represents the state of the _SHIFT_ key for certain kinds of [`Key`]s.
#[derive(Eq, PartialEq, Hash, Debug)]
pub enum Shift {
    Off,
    On,
}

/// Represents the state of the _CONTROL_ key for certain kinds of [`Key`]s.
#[derive(Eq, PartialEq, Hash, Debug)]
pub enum Ctrl {
    Off,
    On,
}

/// A keyboard that reads bytes from the terminal and produces corresponding [`Key`]s.
pub struct Keyboard {
    term: Bytes<Stdin>,
}

impl Keyboard {
    /// Creates a new keyboard reader.
    pub fn new() -> Keyboard {
        Keyboard {
            term: io::stdin().bytes(),
        }
    }

    fn next(&mut self) -> Result<Option<u8>> {
        Ok(self.term.next().transpose()?)
    }

    /// Reads the next key.
    ///
    /// Reads one or more bytes from the underlying terminal and returns the corresponding [`Key`].
    ///
    /// A value of [`Key::None`] will be returned under any of the following conditions:
    ///
    /// - no bytes are available to read after waiting for `1/10` second
    /// - a byte or sequence of bytes is unrecognized
    /// - a byte or sequence of bytes is malformed, such as a `UTF-8` character
    ///
    /// A keyboard assumes that characters from standard input are encoded as `UTF-8`. Any other
    /// encoding will yield unpredictable results in the form of keys that may not be expected.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if an I/O error occurred while reading bytes from the underlying terminal.
    pub fn read(&mut self) -> Result<Key> {
        let key = match self.next()? {
            Some(27) => self.read_escape()?,
            Some(b @ 0..=31) => Key::Control(b),
            Some(b @ 32..=126) => Key::Char(b as char),
            Some(127) => Key::Delete,
            Some(b) => self.read_unicode(b)?,
            None => Key::None,
        };
        Ok(key)
    }

    /// Reads a sequence of bytes prefixed with `ESC`.
    ///
    /// In most cases, this reads an ANSI escape sequence. However, it may produce
    /// [`Key::Control(27)`] itself if no further bytes are read, or [`Key::None`] if the
    /// sequence is unrecognized.
    fn read_escape(&mut self) -> Result<Key> {
        let key = match self.next()? {
            Some(27) => self.read_escape()?,
            Some(b'[') => self.read_ansi()?,
            Some(b'O') => {
                match self.next()? {
                    // F1-F4
                    Some(b @ b'P'..=b'S') => Key::Function(b - b'P' + 1),
                    _ => Key::None,
                }
            }
            None => Key::Control(27),
            _ => Key::None,
        };
        Ok(key)
    }

    /// Reads a sequence of bytes prefixed with `ESC [`.
    ///
    /// Note that this function will interpret the most common sequences only. It is possible that
    /// some well-formed ANSI sequences will be ignored because they simply are not recognized. If
    /// the sequence is unrecognized or malformed, then [`Key::None`] is returned.
    fn read_ansi(&mut self) -> Result<Key> {
        // Optional key code or key modifier depending on trailing byte, which
        // indicates either VT or xterm sequence.
        let (key_code, next_b) = match self.next()? {
            Some(b @ b'0'..=b'9') => {
                let (mut n, next_b) = self.read_number(b)?;
                if n == 0 {
                    n = 1;
                }
                (n, next_b)
            }
            b => (1, b),
        };

        // Optional key modifier, which is bitmask.
        let (key_mod, next_b) = match next_b {
            Some(b';') => match self.next()? {
                Some(b @ b'0'..=b'9') => {
                    let (mut n, next_b) = self.read_number(b)?;
                    if n == 0 {
                        n = 1;
                    }
                    (n, next_b)
                }
                b => (1, b),
            },
            b => (1, b),
        };

        let key = match next_b {
            Some(b'~') => map_vt(key_code, key_mod),
            Some(b) => map_xterm(b, key_mod),
            None => Key::None,
        };
        Ok(key)
    }

    /// Reads a number with a maximum of 2 digits whose first digit is `b`.
    ///
    /// Returns a tuple containing the number itself and the next byte read from the terminal.
    fn read_number(&mut self, b: u8) -> Result<(u8, Option<u8>)> {
        let n = b - b'0';
        let result = match self.next()? {
            Some(b @ b'0'..=b'9') => (n * 10 + (b - b'0'), self.next()?),
            None => (n, self.next()?),
            b => (n, b),
        };
        Ok(result)
    }

    /// Reads a `UTF-8` sequence of bytes where `b` if the first byte.
    ///
    /// `UTF-8` encoding is strictly limited to 2-4 bytes, so anything outside this range
    /// is considered malformed, yielding [`Key::None`].
    fn read_unicode(&mut self, b: u8) -> Result<Key> {
        let n = b.leading_ones() as usize;
        let key = if n < 2 || n > 4 {
            Key::None
        } else {
            let mut buf = [0; 4];
            buf[0] = b;
            for i in 1..n {
                if let Some(b) = self.next()? {
                    buf[i] = b;
                } else {
                    // Expected number of bytes not read, so assumed to be malformed.
                    return Ok(Key::None);
                }
            }
            match from_utf8(&buf[..n])?.chars().next() {
                Some(c) => Key::Char(c),
                None => Key::None,
            }
        };
        Ok(key)
    }
}

/// Returns the key corresponding to the VT-style key code and key modifier, or [`Key::None`] if
/// unrecognized.
fn map_vt(key_code: u8, key_mod: u8) -> Key {
    match (key_code, modifiers(key_mod)) {
        (1, (shift, ctrl)) => Key::Home(shift, ctrl),
        (2, _) => Key::Insert,
        (3, _) => Key::Delete,
        (4, (shift, ctrl)) => Key::End(shift, ctrl),
        (5, (shift, ctrl)) => Key::PageUp(shift, ctrl),
        (6, (shift, ctrl)) => Key::PageDown(shift, ctrl),
        (7, (shift, ctrl)) => Key::Home(shift, ctrl),
        (8, (shift, ctrl)) => Key::End(shift, ctrl),
        // F0-F5
        (code @ 10..=15, _) => Key::Function(code - 10),
        // F6-F10
        (code @ 17..=21, _) => Key::Function(code - 11),
        // F11-F14
        (code @ 23..=26, _) => Key::Function(code - 12),
        // F15-F16
        (code @ 28..=29, _) => Key::Function(code - 13),
        // F17-F20
        (code @ 31..=34, _) => Key::Function(code - 14),
        _ => Key::None,
    }
}

/// Returns the key corresponding to the xterm-style key code and key modifier, or [`Key::None`] if
/// unrecognized.
fn map_xterm(key_code: u8, key_mod: u8) -> Key {
    match (key_code, modifiers(key_mod)) {
        (b'A', (shift, ctrl)) => Key::Up(shift, ctrl),
        (b'B', (shift, ctrl)) => Key::Down(shift, ctrl),
        (b'C', (shift, ctrl)) => Key::Right(shift, ctrl),
        (b'D', (shift, ctrl)) => Key::Left(shift, ctrl),
        (b'F', (shift, ctrl)) => Key::End(shift, ctrl),
        (b'H', (shift, ctrl)) => Key::Home(shift, ctrl),
        (b'Z', _) => Key::ShiftTab,
        // F1-F4
        (code @ b'P'..=b'S', _) => Key::Function(code - b'P' + 1),
        _ => Key::None,
    }
}

// Bitmasks for each type of recognized key modifier per ANSI standard. Note that for sake of
// simplicity, only SHIFT and CONTROL keys are recognized.
const MOD_SHIFT_MASK: u8 = 0x01;
const MOD_CONTROL_MASK: u8 = 0x04;
const MOD_ALL_MASK: u8 = MOD_SHIFT_MASK | MOD_CONTROL_MASK;

/// Returns the state of _SHIFT_ and _CONTROL_ keys based on the given bitmask.
fn modifiers(key_mod: u8) -> (Shift, Ctrl) {
    // Per ANSI standard, all key modifiers default to 1, hence the reason for substraction
    // before applying the bitmask.
    match (key_mod - 1) & MOD_ALL_MASK {
        MOD_SHIFT_MASK => (Shift::On, Ctrl::Off),
        MOD_CONTROL_MASK => (Shift::Off, Ctrl::On),
        MOD_ALL_MASK => (Shift::On, Ctrl::On),
        _ => (Shift::Off, Ctrl::Off),
    }
}
