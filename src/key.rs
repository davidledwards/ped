//! An abstraction over terminal input.

use crate::error::{Error, Result};
use std::cmp;
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
    ScrollUp(Shift, u32, u32),
    ScrollDown(Shift, u32, u32),
    ScrollLeft(Shift, u32, u32),
    ScrollRight(Shift, u32, u32),
    ButtonPress(u32, u32),
    ButtonRelease(u32, u32),
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
pub const TAB: Key = Key::Control(9);
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
            Key::Up(shift, ctrl) => format!("{shift}{ctrl}up"),
            Key::Down(shift, ctrl) => format!("{shift}{ctrl}down"),
            Key::Left(shift, ctrl) => format!("{shift}{ctrl}left"),
            Key::Right(shift, ctrl) => format!("{shift}{ctrl}right"),
            Key::Home(shift, ctrl) => format!("{shift}{ctrl}home"),
            Key::End(shift, ctrl) => format!("{shift}{ctrl}end"),
            Key::PageUp(shift, ctrl) => format!("{shift}{ctrl}pg_up"),
            Key::PageDown(shift, ctrl) => format!("{shift}{ctrl}pg_down"),
            Key::Function(n) => format!("F{n}"),
            Key::ScrollUp(shift, row, col) => format!("{shift}sc_up({row},{col})"),
            Key::ScrollDown(shift, row, col) => format!("{shift}sc_down({row},{col})"),
            Key::ScrollLeft(shift, row, col) => format!("{shift}sc_left({row},{col})"),
            Key::ScrollRight(shift, row, col) => format!("{shift}sc_right({row},{col})"),
            Key::ButtonPress(row, col) => format!("bn_press({row},{col})"),
            Key::ButtonRelease(row, col) => format!("bn_release({row},{col})"),
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for Shift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Shift::Off => "",
            Shift::On => "S-",
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for Ctrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Ctrl::Off => "",
            Ctrl::On => "C-",
        };
        write!(f, "{s}")
    }
}

/// Wrapper used only for formatting [`Key::Control`] values.
struct Control(u8);

impl Control {
    /// Mapping of control codes to display character, excluding DEL (^?), which is
    /// handled separately.
    const CONTROL_CHAR: [char; 32] = [
        '@', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
        'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '[', '\\', ']', '^', '_',
    ];
}

impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            9 => write!(f, "TAB"),
            13 => write!(f, "RET"),
            27 => write!(f, "ESC"),
            127 => write!(f, "DEL"),
            b @ 0..32 => write!(f, "C-{}", Self::CONTROL_CHAR[b as usize]),
            b @ _ => {
                // This should never happen, but nonetheless, format as number.
                write!(f, "C-#{b}")
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

    /// Reads the next key.
    ///
    /// Reads one or more bytes from the underlying terminal and returns the
    /// corresponding [`Key`].
    ///
    /// A value of [`Key::None`] will be returned under any of the following conditions:
    ///
    /// - no bytes are available to read after waiting for `1/10` second
    /// - a byte or sequence of bytes is unrecognized
    /// - a byte or sequence of bytes is malformed, such as a `UTF-8` character
    ///
    /// A keyboard assumes that characters from standard input are encoded as `UTF-8`.
    /// Any other encoding will yield unpredictable results in the form of keys that may
    /// not be expected.
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
    /// [`Key::Control(27)`] itself if no further bytes are read, or [`Key::None`] if
    /// the sequence is unrecognized.
    fn read_escape(&mut self) -> Result<Key> {
        let key = match self.next()? {
            Some(b'[') => self.read_ansi()?,
            Some(b'O') => self.read_fn()?,
            Some(b) => {
                self.push_back(b);
                Key::Control(27)
            }
            None => Key::Control(27),
        };
        Ok(key)
    }

    /// Reads a sequence of bytes prefixed with `ESC O`.
    ///
    /// Only the first four function keys are encoded in this manner.
    fn read_fn(&mut self) -> Result<Key> {
        let key = match self.next()? {
            // F1-F4
            Some(b @ b'P'..=b'S') => Key::Function(b - b'P' + 1),
            _ => Key::None,
        };
        Ok(key)
    }

    /// Reads a sequence of bytes prefixed with `ESC [`.
    ///
    /// Note that this function will interpret the most common sequences only. It is
    /// possible that some well-formed ANSI sequences will be ignored because they
    /// simply are not recognized. If the sequence is unrecognized or malformed, then
    /// [`Key::None`] is returned.
    fn read_ansi(&mut self) -> Result<Key> {
        let key = match self.next()? {
            Some(b'<') => self.read_mouse()?,
            Some(b) => self.push_back(b).read_key()?,
            None => Key::None,
        };
        Ok(key)
    }

    /// Reads a VT or xterm key sequence prefixed with `ESC [`.
    fn read_key(&mut self) -> Result<Key> {
        // Optional key code or key modifier depending on trailing byte.
        let key_code = match self.read_number()? {
            Some(n) => cmp::max(1, n),
            None => 1,
        } as u8;

        // Optional key modifier, which is bitmask.
        let key_mod = if let Some(_) = self.read_literal(&[b';'])? {
            match self.read_number()? {
                Some(n) => cmp::max(1, n),
                None => 1,
            }
        } else {
            1
        } as u8;

        let key = match self.next()? {
            Some(b'~') => map_vt(key_code, key_mod),
            Some(b) => map_xterm(b, key_mod),
            None => Key::None,
        };
        Ok(key)
    }

    /// Reads a mouse sequence prefixed with `ESC [<`.
    fn read_mouse(&mut self) -> Result<Key> {
        let button = match self.read_number()? {
            Some(button) => button,
            None => return Ok(Key::None),
        };

        let col = if let Some(_) = self.read_literal(&[b';'])? {
            match self.read_number()? {
                Some(col) => {
                    if col > 0 {
                        col - 1
                    } else {
                        0
                    }
                }
                None => return Ok(Key::None),
            }
        } else {
            return Ok(Key::None);
        };

        let row = if let Some(_) = self.read_literal(&[b';'])? {
            match self.read_number()? {
                Some(row) => {
                    if row > 0 {
                        row - 1
                    } else {
                        0
                    }
                }
                None => return Ok(Key::None),
            }
        } else {
            return Ok(Key::None);
        };

        let key = if let Some(b) = self.read_literal(&[b'M', b'm'])? {
            if button & 64 == 0 {
                if b == b'M' {
                    Key::ButtonPress(row, col)
                } else {
                    Key::ButtonRelease(row, col)
                }
            } else {
                let shift = if button & 4 == 0 {
                    Shift::Off
                } else {
                    Shift::On
                };
                match button & 3 {
                    0 => Key::ScrollUp(shift, row, col),
                    1 => Key::ScrollDown(shift, row, col),
                    2 => Key::ScrollRight(shift, row, col),
                    3 => Key::ScrollLeft(shift, row, col),
                    _ => Key::None,
                }
            }
        } else {
            Key::None
        };
        Ok(key)
    }

    /// Reads a `UTF-8` sequence of bytes where `b` if the first byte.
    ///
    /// `UTF-8` encoding is strictly limited to 2-4 bytes, so anything outside this
    /// range is considered malformed, yielding [`Key::None`].
    fn read_unicode(&mut self, b: u8) -> Result<Key> {
        let n = b.leading_ones() as usize;
        let key = if n < 2 || n > 4 {
            Key::None
        } else {
            let mut buf = [b, 0, 0, 0];
            for i in 1..n {
                if let Some(b) = self.next()? {
                    buf[i] = b;
                } else {
                    return Ok(Key::None);
                }
            }
            to_utf8(&buf[..n])?
                .chars()
                .next()
                .map(|c| Key::Char(c))
                .unwrap_or(Key::None)
        };
        Ok(key)
    }

    /// Reads a number until a non-ASCII-digit character is encountered.
    fn read_number(&mut self) -> Result<Option<u32>> {
        let mut n: u32 = 0;
        while let Some(digit) = self.read_digit()? {
            n = n.saturating_mul(10).saturating_add(digit);
        }
        Ok(Some(n))
    }

    /// Reads the next byte and expects it to match one of the bytes in `lits`,
    /// returning the matching byte, otherwise `None`.
    fn read_literal(&mut self, lits: &[u8]) -> Result<Option<u8>> {
        let c = self.next()?.and_then(|b| {
            if lits.contains(&b) {
                Some(b)
            } else {
                self.push_back(b);
                None
            }
        });
        Ok(c)
    }

    /// Returns the next byte if it matches an ASCII digit, otherwise `None`.
    fn read_digit(&mut self) -> Result<Option<u32>> {
        let digit = self.next()?.and_then(|b| {
            if is_digit(b) {
                Some((b - b'0') as u32)
            } else {
                self.push_back(b);
                None
            }
        });
        Ok(digit)
    }

    /// Reads the next byte from `stdin` or `None` if no bytes are available to read.
    fn next(&mut self) -> Result<Option<u8>> {
        if let Some(b) = self.stdin_waiting.take() {
            Ok(Some(b))
        } else {
            self.stdin
                .next()
                .transpose()
                .map_err(|e| Error::io("/dev/stdin", e))
        }
    }

    /// Push back `b` as if it were not read from `stdin`.
    ///
    /// A subsequent call to [`next()`](Self::next) will return `b`.
    fn push_back(&mut self, b: u8) -> &mut Self {
        self.stdin_waiting = Some(b);
        self
    }
}

/// Returns `true` if `b` is an ASCII digit.
#[inline(always)]
fn is_digit(b: u8) -> bool {
    b >= b'0' && b <= b'9'
}

/// Converts the UTF-8 sequence in `buf` to a valid string slice.
fn to_utf8(buf: &[u8]) -> Result<&str> {
    str::from_utf8(buf).map_err(|e| Error::utf8(buf, e))
}

/// Returns the key corresponding to the VT-style key code and key modifier, or
/// [`Key::None`] if unrecognized.
fn map_vt(key_code: u8, key_mod: u8) -> Key {
    match (key_code, map_mods(key_mod)) {
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

/// Returns the key corresponding to the xterm-style key code and key modifier,
/// or [`Key::None`] if unrecognized.
fn map_xterm(key_code: u8, key_mod: u8) -> Key {
    match (key_code, map_mods(key_mod)) {
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
fn map_mods(key_mod: u8) -> (Shift, Ctrl) {
    // Bitmasks for each type of recognized key modifier per ANSI standard. Note
    // that for sake of simplicity, only SHIFT and CONTROL keys are recognized.
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

/// Returns a string constructed by joining the result of [`pretty_keys`] with the
/// space character.
pub fn pretty(keys: &Vec<Key>) -> String {
    pretty_keys(keys).join(" ")
}

/// Returns a vector of key names extracted from `keys`, wheressequences of
/// `ESC` + `<key>` are replaced with `M-<key>`.
pub fn pretty_keys(keys: &Vec<Key>) -> Vec<String> {
    let mut keys = keys.iter().map(|key| key.to_string()).collect::<Vec<_>>();
    if keys.len() > 1 {
        let mut i = keys.len() - 1;
        while i > 0 {
            if keys[i - 1] == "ESC" {
                let key = format!("M-{}", keys[i]);
                keys.drain(i - 1..=i);
                keys.insert(i - 1, key);
            }
            i -= 1;
        }
    }
    keys
}

/// Predefined mapping of key names to [`Key`]s.
///
/// A few special keys are bound to multiple names as a convenience.
///
/// Note that [`Key::Char`] is absent from these mappings because of the impracticality
/// of mapping all possible characters.
pub const KEY_MAPPINGS: [(&'static str, Key); 90] = [
    ("C-@", Key::Control(0)),
    ("C-a", Key::Control(1)),
    ("C-b", Key::Control(2)),
    ("C-c", Key::Control(3)),
    ("C-d", Key::Control(4)),
    ("C-e", Key::Control(5)),
    ("C-f", Key::Control(6)),
    ("C-g", Key::Control(7)),
    ("C-h", Key::Control(8)),
    ("C-i", Key::Control(9)),
    ("tab", Key::Control(9)),
    ("C-j", Key::Control(10)),
    ("C-k", Key::Control(11)),
    ("C-l", Key::Control(12)),
    ("C-m", Key::Control(13)),
    ("ret", Key::Control(13)),
    ("C-n", Key::Control(14)),
    ("C-o", Key::Control(15)),
    ("C-p", Key::Control(16)),
    ("C-q", Key::Control(17)),
    ("C-r", Key::Control(18)),
    ("C-s", Key::Control(19)),
    ("C-t", Key::Control(20)),
    ("C-u", Key::Control(21)),
    ("C-v", Key::Control(22)),
    ("C-w", Key::Control(23)),
    ("C-x", Key::Control(24)),
    ("C-y", Key::Control(25)),
    ("C-z", Key::Control(26)),
    ("C-[", Key::Control(27)),
    ("ESC", Key::Control(27)),
    ("C-\\", Key::Control(28)),
    ("C-]", Key::Control(29)),
    ("C-^", Key::Control(30)),
    ("C-_", Key::Control(31)),
    ("C-?", Key::Control(127)),
    ("del", Key::Control(127)),
    ("S-tab", Key::ShiftTab),
    ("up", Key::Up(Shift::Off, Ctrl::Off)),
    ("S-up", Key::Up(Shift::On, Ctrl::Off)),
    ("C-up", Key::Up(Shift::Off, Ctrl::On)),
    ("S-C-up", Key::Up(Shift::On, Ctrl::On)),
    ("down", Key::Down(Shift::Off, Ctrl::Off)),
    ("S-down", Key::Down(Shift::On, Ctrl::Off)),
    ("C-down", Key::Down(Shift::Off, Ctrl::On)),
    ("S-C-down", Key::Down(Shift::On, Ctrl::On)),
    ("left", Key::Left(Shift::Off, Ctrl::Off)),
    ("S-left", Key::Left(Shift::On, Ctrl::Off)),
    ("C-left", Key::Left(Shift::Off, Ctrl::On)),
    ("S-C-left", Key::Left(Shift::On, Ctrl::On)),
    ("right", Key::Right(Shift::Off, Ctrl::Off)),
    ("S-right", Key::Right(Shift::On, Ctrl::Off)),
    ("C-right", Key::Right(Shift::Off, Ctrl::On)),
    ("S-C-right", Key::Right(Shift::On, Ctrl::On)),
    ("home", Key::Home(Shift::Off, Ctrl::Off)),
    ("S-home", Key::Home(Shift::On, Ctrl::Off)),
    ("C-home", Key::Home(Shift::Off, Ctrl::On)),
    ("S-C-home", Key::Home(Shift::On, Ctrl::On)),
    ("end", Key::End(Shift::Off, Ctrl::Off)),
    ("S-end", Key::End(Shift::On, Ctrl::Off)),
    ("C-end", Key::End(Shift::Off, Ctrl::On)),
    ("S-C-end", Key::End(Shift::On, Ctrl::On)),
    ("pg_up", Key::PageUp(Shift::Off, Ctrl::Off)),
    ("S-pg_up", Key::PageUp(Shift::On, Ctrl::Off)),
    ("C-pg_up", Key::PageUp(Shift::Off, Ctrl::On)),
    ("S-C-pg_up", Key::PageUp(Shift::On, Ctrl::On)),
    ("pg_down", Key::PageDown(Shift::Off, Ctrl::Off)),
    ("S-pg_down", Key::PageDown(Shift::On, Ctrl::Off)),
    ("C-pg_down", Key::PageDown(Shift::Off, Ctrl::On)),
    ("S-C-pg_down", Key::PageDown(Shift::On, Ctrl::On)),
    ("F1", Key::Function(1)),
    ("F2", Key::Function(2)),
    ("F3", Key::Function(3)),
    ("F4", Key::Function(4)),
    ("F5", Key::Function(5)),
    ("F6", Key::Function(6)),
    ("F7", Key::Function(7)),
    ("F8", Key::Function(8)),
    ("F9", Key::Function(9)),
    ("F10", Key::Function(10)),
    ("F11", Key::Function(11)),
    ("F12", Key::Function(12)),
    ("F13", Key::Function(13)),
    ("F14", Key::Function(14)),
    ("F15", Key::Function(15)),
    ("F16", Key::Function(16)),
    ("F17", Key::Function(17)),
    ("F18", Key::Function(18)),
    ("F19", Key::Function(19)),
    ("F20", Key::Function(20)),
];

/// Returns a mapping of key names to [`Key`]s.
pub fn init_key_map() -> KeyMap {
    let mut key_map = KeyMap::new();
    for (name, key) in KEY_MAPPINGS {
        key_map.insert(name, key);
    }
    key_map
}
