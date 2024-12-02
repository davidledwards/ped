//! A representation of color.
//!
//! Colors follow the ANSI 8-bit standard, which is referenced
//! [here](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit).

/// A combination of *foreground* and *background* color.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Color {
    pub fg: u8,
    pub bg: u8,
}

impl Color {
    /// A special color constant where `fg` and `bg` are initialized to `0`.
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
