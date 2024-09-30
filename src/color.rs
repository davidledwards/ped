//! Text colors.
//!
//! Colors follow the ANSI 8-bit standard, which is referenced
//! [here](https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences).
use std::collections::HashMap;

/// An encapsulation of *foreground* and *background* colors.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Color {
    pub fg: u8,
    pub bg: u8,
}

/// Map of canonical color names to ANSI color values.
type Colors = HashMap<&'static str, u8>;

pub struct ColorMap {
    colors: Colors,
}

impl Color {
    pub const ZERO: Color = Color::new(0, 0);

    pub const fn new(fg: u8, bg: u8) -> Color {
        Color { fg, bg }
    }
}

impl Default for Color {
    fn default() -> Color {
        Color::ZERO
    }
}

impl ColorMap {
    pub fn new() -> ColorMap {
        ColorMap {
            colors: init_colors(),
        }
    }

    pub fn find(&self, name: &str) -> Option<u8> {
        self.colors.get(name).copied()
    }
}

/// Predefined color mappings that associate canonical names to ANSI color values.
const COLOR_MAPPINGS: [(&'static str, u8); 16] = [
    ("black", 0),
    ("red", 1),
    ("green", 2),
    ("yellow", 3),
    ("blue", 4),
    ("magenta", 5),
    ("cyan", 6),
    ("white", 7),
    ("gray", 8),
    ("bright-red", 9),
    ("bright-green", 10),
    ("bright-yellow", 11),
    ("bright-blue", 12),
    ("bright-magenta", 13),
    ("bright-cyan", 14),
    ("bright-white", 15),
];

fn init_colors() -> Colors {
    let mut colors = Colors::new();
    for (name, value) in COLOR_MAPPINGS {
        colors.insert(name, value);
    }
    colors
}
