//! A collection of functions that produce ANSI control sequences used in the
//! rendering of terminal output.
//!
//! Of particular note, ANSI sequences related to cursor positioning are `1`-based,
//! so functions in this module that accept *row* and *column* are presumed to be
//! `0`-based and silently add `1` to values.

use crate::color::Color;
use crate::size::Point;

pub fn clear_screen() -> &'static str {
    "\x1b[2J\x1b[H"
}

pub fn show_cursor() -> &'static str {
    "\x1b[?25h"
}

pub fn hide_cursor() -> &'static str {
    "\x1b[?25l"
}

pub fn set_cursor(p: Point) -> String {
    format!("\x1b[{};{}H", p.row + 1, p.col + 1)
}

pub fn set_color(color: Color) -> String {
    format!("\x1b[38;5;{}m\x1b[48;5;{}m", color.fg, color.bg)
}
