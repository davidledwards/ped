//! ANSI escape sequences.
use crate::color::Color;
use crate::size::Point;

pub fn show_cursor() -> &'static str {
    "\x1b[?25h"
}

pub fn hide_cursor() -> &'static str {
    "\x1b[?25l"
}

pub fn set_cursor(p: Point) -> String {
    // Note that row and col are 0-based even though ANSI sequence itself is 1-based.
    format!("\x1b[{};{}H", p.row + 1, p.col + 1)
}

pub fn set_color(color: Color) -> String {
    format!("\x1b[38;5;{}m\x1b[48;5;{}m", color.fg, color.bg)
}
