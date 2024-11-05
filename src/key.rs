//! Keyboard reader.
use crate::error::{Error, Result};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Bytes, Read, Stdin};
use std::str;

/// The set of keys recognized by [`Keyboard`]s.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Key {
    None,
    Control(u8),
    Char(char),
    ShiftTab,
    Up(Shift, Ctrl),
    Down(Shift, Ctrl),
    Left(Shift, Ctrl),
    Right(Shift, Ctrl),
    Home(Shift, Ctrl),
    End(Shift, Ctrl),
    PageUp(Shift, Ctrl),
    PageDown(Shift, Ctrl),
    Function(u8),
}

/// Represents the state of the _SHIFT_ key for certain kinds of [`Key`]s.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Shift {
    Off,
    On,
}

/// Represents the state of the _CONTROL_ key for certain kinds of [`Key`]s.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Ctrl {
    Off,
    On,
}

// Various predefined keys.
pub const CTRL_A: Key = Key::Control(1);
pub const CTRL_B: Key = Key::Control(2);
pub const CTRL_D: Key = Key::Control(4);
pub const CTRL_E: Key = Key::Control(5);
pub const CTRL_F: Key = Key::Control(6);
pub const CTRL_G: Key = Key::Control(7);
pub const CTRL_H: Key = Key::Control(8);
pub const CTRL_J: Key = Key::Control(10);
pub const CTRL_K: Key = Key::Control(11);
pub const CTRL_M: Key = Key::Control(13);
pub const DELETE: Key = Key::Control(127);
pub const LEFT: Key = Key::Left(Shift::Off, Ctrl::Off);
pub const RIGHT: Key = Key::Right(Shift::Off, Ctrl::Off);
pub const HOME: Key = Key::Home(Shift::Off, Ctrl::Off);
pub const END: Key = Key::End(Shift::Off, Ctrl::Off);

/// Map of key names to [`Key`]s.
pub type KeyMap = HashMap<&'static str, Key>;

/// A keyboard that reads bytes from the terminal and produces corresponding [`Key`]s.
pub struct Keyboard {
    /// A non-blocking stream of bytes from standard input.
    stdin: Bytes<Stdin>,

    /// An optional byte previously read but pushed back for processing.
    stdin_waiting: Option<u8>,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Key::None => "<none>".to_string(),
            Key::Control(b) => format!("{}", Control(*b)),
            Key::Char(c) => format!("{c}"),
            Key::ShiftTab => format!("{}{}", Shift::On, Control(9)),
            Key::Up(shift, ctrl) => format!("{shift}{ctrl}<up>"),
            Key::Down(shift, ctrl) => format!("{shift}{ctrl}<down>"),
            Key::Left(shift, ctrl) => format!("{shift}{ctrl}<left>"),
            Key::Right(shift, ctrl) => format!("{shift}{ctrl}<right>"),
            Key::Home(shift, ctrl) => format!("{shift}{ctrl}<home>"),
            Key::End(shift, ctrl) => format!("{shift}{ctrl}<end>"),
            Key::PageUp(shift, ctrl) => format!("{shift}{ctrl}<page-up>"),
            Key::PageDown(shift, ctrl) => format!("{shift}{ctrl}<page-down>"),
            Key::Function(n) => format!("F{n}"),
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for Shift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Shift::Off => "",
            Shift::On => "\\",
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for Ctrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Ctrl::Off => "",
            Ctrl::On => "^",
        };
        write!(f, "{s}")
    }
}

/// Wrapper used only for formatting [`Key::Control`] values.
struct Control(u8);

impl Control {
    /// Mapping of control codes to display character, excluding DEL (^?), which is handled separately.
    const CONTROL_CHAR: [char; 32] = [
        '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q',
        'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_',
    ];
}

impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            27 => write!(f, "<esc>"),
            127 => write!(f, "^?"),
            b @ 0..32 => write!(f, "^{}", Self::CONTROL_CHAR[b as usize]),
            b @ _ => {
                // This should never happen, but nonetheless, format as number.
                write!(f, "^#{b}")
            }
        }
    }
}

impl Keyboard {
    /// Creates a new keyboard reader.
    pub fn new() -> Keyboard {
        Keyboard {
            stdin: io::stdin().bytes(),
            stdin_waiting: None,
        }
    }

    fn next(&mut self) -> Result<Option<u8>> {
        if let Some(b) = self.stdin_waiting.take() {
            Ok(Some(b))
        } else {
            self.stdin
                .next()
                .transpose()
                .map_err(|e| Error::io(Some("/dev/stdin"), e))
        }
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
            Some(b @ 0..32) => Key::Control(b),
            Some(b @ 32..127) => Key::Char(b as char),
            Some(b @ 127) => Key::Control(b),
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
            Some(b'[') => self.read_ansi()?,
            Some(b'O') => {
                match self.next()? {
                    // F1-F4
                    Some(b @ b'P'..=b'S') => Key::Function(b - b'P' + 1),
                    _ => Key::None,
                }
            }
            Some(b) => {
                self.stdin_waiting = Some(b);
                Key::Control(27)
            }
            None => Key::Control(27),
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
            Some(b'~') => Self::map_vt(key_code, key_mod),
            Some(b) => Self::map_xterm(b, key_mod),
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
            match Self::to_utf8(&buf[..n])?.chars().next() {
                Some(c) => Key::Char(c),
                None => Key::None,
            }
        };
        Ok(key)
    }

    /// Returns the key corresponding to the VT-style key code and key modifier, or [`Key::None`] if
    /// unrecognized.
    fn map_vt(key_code: u8, key_mod: u8) -> Key {
        match (key_code, Self::modifiers(key_mod)) {
            (1, (shift, ctrl)) => Key::Home(shift, ctrl),
            (2, _) => {
                // INS key, but for now, just ignore.
                Key::None
            }
            (3, _) => Key::Control(127),
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
        match (key_code, Self::modifiers(key_mod)) {
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

    /// Returns the state of _SHIFT_ and _CONTROL_ keys based on the given bitmask.
    fn modifiers(key_mod: u8) -> (Shift, Ctrl) {
        // Bitmasks for each type of recognized key modifier per ANSI standard. Note that
        // for sake of simplicity, only SHIFT and CONTROL keys are recognized.
        const MOD_SHIFT_MASK: u8 = 0x01;
        const MOD_CONTROL_MASK: u8 = 0x04;
        const MOD_ALL_MASK: u8 = MOD_SHIFT_MASK | MOD_CONTROL_MASK;

        // Per ANSI standard, all key modifiers default to 1, hence the reason for
        // substraction before applying the bitmask.
        match (key_mod - 1) & MOD_ALL_MASK {
            MOD_SHIFT_MASK => (Shift::On, Ctrl::Off),
            MOD_CONTROL_MASK => (Shift::Off, Ctrl::On),
            MOD_ALL_MASK => (Shift::On, Ctrl::On),
            _ => (Shift::Off, Ctrl::Off),
        }
    }

    /// Converts the UTF-8 sequence in `buf` to a valid string slice.
    fn to_utf8(buf: &[u8]) -> Result<&str> {
        str::from_utf8(buf).map_err(|e| Error::utf8(buf, e))
    }
}

/// Predefined mapping of key names to [`Key`]s.
///
/// Note that [`Key::Char`] is absent from these mappings because of the impracticality
/// of mapping all possible characters.
const KEY_MAPPINGS: [(&'static str, Key); 86] = [
    ("ctrl-@", Key::Control(0)),
    ("ctrl-a", Key::Control(1)),
    ("ctrl-b", Key::Control(2)),
    ("ctrl-c", Key::Control(3)),
    ("ctrl-d", Key::Control(4)),
    ("ctrl-e", Key::Control(5)),
    ("ctrl-f", Key::Control(6)),
    ("ctrl-g", Key::Control(7)),
    ("ctrl-h", Key::Control(8)),
    ("ctrl-i", Key::Control(9)),
    ("ctrl-j", Key::Control(10)),
    ("ctrl-k", Key::Control(11)),
    ("ctrl-l", Key::Control(12)),
    ("ctrl-m", Key::Control(13)),
    ("ctrl-n", Key::Control(14)),
    ("ctrl-o", Key::Control(15)),
    ("ctrl-p", Key::Control(16)),
    ("ctrl-q", Key::Control(17)),
    ("ctrl-r", Key::Control(18)),
    ("ctrl-s", Key::Control(19)),
    ("ctrl-t", Key::Control(20)),
    ("ctrl-u", Key::Control(21)),
    ("ctrl-v", Key::Control(22)),
    ("ctrl-w", Key::Control(23)),
    ("ctrl-x", Key::Control(24)),
    ("ctrl-y", Key::Control(25)),
    ("ctrl-z", Key::Control(26)),
    ("ctrl-[", Key::Control(27)),
    ("ctrl-\\", Key::Control(28)),
    ("ctrl-]", Key::Control(29)),
    ("ctrl-^", Key::Control(30)),
    ("ctrl-_", Key::Control(31)),
    ("ctrl-?", Key::Control(127)),
    ("shift-tab", Key::ShiftTab),
    ("up", Key::Up(Shift::Off, Ctrl::Off)),
    ("shift-up", Key::Up(Shift::On, Ctrl::Off)),
    ("ctrl-up", Key::Up(Shift::Off, Ctrl::On)),
    ("shift-ctrl-up", Key::Up(Shift::On, Ctrl::On)),
    ("down", Key::Down(Shift::Off, Ctrl::Off)),
    ("shift-down", Key::Down(Shift::On, Ctrl::Off)),
    ("ctrl-down", Key::Down(Shift::Off, Ctrl::On)),
    ("shift-ctrl-down", Key::Down(Shift::On, Ctrl::On)),
    ("left", Key::Left(Shift::Off, Ctrl::Off)),
    ("shift-left", Key::Left(Shift::On, Ctrl::Off)),
    ("ctrl-left", Key::Left(Shift::Off, Ctrl::On)),
    ("shift-ctrl-left", Key::Left(Shift::On, Ctrl::On)),
    ("right", Key::Right(Shift::Off, Ctrl::Off)),
    ("shift-right", Key::Right(Shift::On, Ctrl::Off)),
    ("ctrl-right", Key::Right(Shift::Off, Ctrl::On)),
    ("shift-ctrl-right", Key::Right(Shift::On, Ctrl::On)),
    ("home", Key::Home(Shift::Off, Ctrl::Off)),
    ("shift-home", Key::Home(Shift::On, Ctrl::Off)),
    ("ctrl-home", Key::Home(Shift::Off, Ctrl::On)),
    ("shift-ctrl-home", Key::Home(Shift::On, Ctrl::On)),
    ("end", Key::End(Shift::Off, Ctrl::Off)),
    ("shift-end", Key::End(Shift::On, Ctrl::Off)),
    ("ctrl-end", Key::End(Shift::Off, Ctrl::On)),
    ("shift-ctrl-end", Key::End(Shift::On, Ctrl::On)),
    ("page-up", Key::PageUp(Shift::Off, Ctrl::Off)),
    ("shift-page-up", Key::PageUp(Shift::On, Ctrl::Off)),
    ("ctrl-page-up", Key::PageUp(Shift::Off, Ctrl::On)),
    ("shift-ctrl-page-up", Key::PageUp(Shift::On, Ctrl::On)),
    ("page-down", Key::PageDown(Shift::Off, Ctrl::Off)),
    ("shift-page-down", Key::PageDown(Shift::On, Ctrl::Off)),
    ("ctrl-page-down", Key::PageDown(Shift::Off, Ctrl::On)),
    ("shift-ctrl-page-down", Key::PageDown(Shift::On, Ctrl::On)),
    ("fn-1", Key::Function(1)),
    ("fn-2", Key::Function(2)),
    ("fn-3", Key::Function(3)),
    ("fn-4", Key::Function(4)),
    ("fn-5", Key::Function(5)),
    ("fn-6", Key::Function(6)),
    ("fn-7", Key::Function(7)),
    ("fn-8", Key::Function(8)),
    ("fn-9", Key::Function(9)),
    ("fn-10", Key::Function(10)),
    ("fn-11", Key::Function(11)),
    ("fn-12", Key::Function(12)),
    ("fn-13", Key::Function(13)),
    ("fn-14", Key::Function(14)),
    ("fn-15", Key::Function(15)),
    ("fn-16", Key::Function(16)),
    ("fn-17", Key::Function(17)),
    ("fn-18", Key::Function(18)),
    ("fn-19", Key::Function(19)),
    ("fn-20", Key::Function(20)),
];

/// Returns a mapping of key names to [`Key`]s.
pub fn init_key_map() -> KeyMap {
    let mut key_map = KeyMap::new();
    for (name, key) in KEY_MAPPINGS {
        key_map.insert(name, key);
    }
    key_map
}
