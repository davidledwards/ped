//! A collection of functions that produce ANSI control sequences used in the
//! rendering of terminal output.
//!
//! Of particular note, ANSI sequences related to cursor positioning are `1`-based,
//! so functions in this module that accept _row_ and _column_ are presumed to be
//! `0`-based and silently add `1` to values.

use crate::color::Color;
use crate::size::Point;

pub fn alt_screen(on: bool) -> &'static str {
    if on { "\x1b[?1049h" } else { "\x1b[?1049l" }
}

pub fn track_mouse(on: bool) -> &'static str {
    if on {
        "\x1b[?1000h\x1b[?1006h"
    } else {
        "\x1b[?1000l\x1b[?1006l"
    }
}

pub fn clear_screen() -> &'static str {
    "\x1b[2J\x1b[H"
}

#[inline]
pub fn show_cursor() -> &'static str {
    "\x1b[?25h"
}

#[inline]
pub fn hide_cursor() -> &'static str {
    "\x1b[?25l"
}

#[inline]
pub fn set_cursor(p: Point) -> String {
    format!("\x1b[{};{}H", p.row + 1, p.col + 1)
}

#[inline]
pub fn set_color(color: Color) -> String {
    format!("\x1b[38;5;{}m\x1b[48;5;{}m", color.fg, color.bg)
}

#[inline]
pub fn set_color_fg(fg: u8) -> String {
    format!("\x1b[38;5;{fg}m")
}

#[inline]
pub fn set_color_bg(bg: u8) -> String {
    format!("\x1b[48;5;{bg}m")
}
