//! Text colors.

// plan is to follow the ANSI 8-bit standard. see article on Wikipedia.
// https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Color {
    pub fg: u8,
    pub bg: u8,
}

impl Color {
    pub const ZERO: Color = Color {
        fg: 0,
        bg: 0,
    };

    pub fn new(fg: u8, bg: u8) -> Color {
        Color { fg, bg }
    }
}

impl Default for Color {
    fn default() -> Color {
        Color::ZERO
    }
}

// predefined colors

pub const BLACK: u8 = 0;
pub const RED: u8 = 1;
pub const GREEN: u8 = 2;
pub const YELLOW: u8 = 3;
pub const BLUE: u8 = 4;
pub const MAGENTA: u8 = 5;
pub const CYAN: u8 = 6;
pub const WHITE: u8 = 7;
pub const GRAY: u8 = 8;
pub const BRIGHT_RED: u8 = 9;
pub const BRIGHT_GREEN: u8 = 10;
pub const BRIGHT_YELLOW: u8 = 11;
pub const BRIGHT_BLUE: u8 = 12;
pub const BRIGHT_MAGENTA: u8 = 13;
pub const BRIGHT_CYAN: u8 = 14;
pub const BRIGHT_WHITE: u8 = 15;
