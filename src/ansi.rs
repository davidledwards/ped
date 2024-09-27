//! ANSI escape sequences.

use crate::color::Color;
use crate::display::Point;

pub const CSI: &str = "\x1b[";

pub fn set_cursor(p: Point) -> String {
    // Note that row and col are 0-based even though ANSI sequence itself is 1-based.
    format!("{CSI}{};{}H", p.row + 1, p.col + 1)
}

pub fn set_fg(fg: u8) -> String {
    format!("{CSI}38;5;{fg}m")
}

pub fn set_bg(bg: u8) -> String {
    format!("{CSI}48;5;{bg}m")
}

pub fn set_color(color: Color) -> String {
    format!("{CSI}38;5;{}m{CSI}48;5;{}m", color.fg, color.bg)
}
