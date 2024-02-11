//! ANSI escape sequences.

pub const CSI: &str = "\x1b[";

// Set location of cursor.
//
// Note that row and col are 0-based even though ANSI sequence itself is 1-based.
pub fn set_cursor(row: usize, col: usize) -> String {
    format!("{CSI}{};{}H", row + 1, col + 1)
}

pub fn set_fg(fg: u8) -> String {
    format!("{CSI}38;5;{fg}m")
}

pub fn set_bg(bg: u8) -> String {
    format!("{CSI}48;5;{bg}m")
}

pub fn set_color(fg: u8, bg: u8) -> String {
    format!("{CSI}38;5;{fg}m{CSI}48;5;{bg}m")
}
