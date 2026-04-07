//! A collection of functions for styling text during the rendering process.

use crate::color::Color;
use crate::config::ConfigurationRef;
use crate::grid::Cell;
use crate::size::Point;
use std::ops::Range;

/// A _styler_ creates [pens](Pen) that are used during the rendering process.
pub struct Styler {
    /// Configuration that dictates colors and behaviors.
    config: ConfigurationRef,

    /// Color of margin.
    margin_color: Color,

    /// Color of current line in margin.
    line_color: Color,

    /// Color of text with no special treatment.
    text_color: Color,

    /// Color of text in margin gutters.
    gutter_color: Color,
}

/// A _pen_ captures state information that is used to format [cells](Cell) during the
/// rendering process.
pub struct Pen<'a> {
    /// A reference to the styler that created this pen.
    styler: &'a Styler,

    /// Current cursor position.
    cursor: Point,

    /// Current `1`-based line number.
    line: u32,

    /// Range in the buffer containing selected text, if applicable, otherwise this
    /// span is assumed to be `0`..`0`.
    selected: Range<usize>,
}

impl Styler {
    /// Creates a new styler using `config`.
    pub fn new(config: ConfigurationRef) -> Styler {
        let margin_color = Color::new(config.theme.margin_fg, config.theme.margin_bg);
        let line_color = Color::new(config.theme.line_fg, config.theme.margin_bg);
        let text_color = Color::new(config.theme.text_fg, config.theme.text_bg);
        let gutter_color = Color::new(config.theme.gutter_fg, config.theme.text_bg);

        Styler {
            config,
            margin_color,
            line_color,
            text_color,
            gutter_color,
        }
    }

    /// Creates a pen.
    ///
    /// State provided by caller:
    /// - `cursor`: current cursor position on display
    /// - `line`: current `1`-based line number in buffer
    /// - `selected`: an optional region of selected text
    pub fn pen(&self, cursor: Point, line: u32, selected: Option<Range<usize>>) -> Pen<'_> {
        Pen::new(self, cursor, line, selected)
    }
}

impl<'a> Pen<'a> {
    /// Special character shown for `\n` (newline) when visible.
    const EOL_CHAR: char = '\u{21b2}';

    /// Special character shown for `\t` (tab).
    const TAB_CHAR: char = '\u{2192}';

    /// Special character shown for all other ASCII control characters.
    const CTRL_CHAR: char = '\u{00bf}';

    fn new(
        styler: &'a Styler,
        cursor: Point,
        line: u32,
        selected: Option<Range<usize>>,
    ) -> Pen<'a> {
        Pen {
            styler,
            cursor,
            line,
            selected: selected.unwrap_or(0..0),
        }
    }

    /// Formats `c` using the margin color.
    #[inline]
    pub fn as_margin(&self, c: char) -> Cell {
        Cell::new(c, self.styler.margin_color)
    }

    /// Formats `c` using the line color if `line` is the current line, otherwise the
    /// margin color is applied.
    #[inline]
    pub fn as_line(&self, c: char, line: u32) -> Cell {
        let color = if line == self.line {
            self.styler.line_color
        } else {
            self.styler.margin_color
        };
        Cell::new(c, color)
    }

    /// Formats ` ` (space) using the text color.
    #[inline]
    pub fn as_blank(&self) -> Cell {
        Cell::new(' ', self.styler.text_color)
    }

    /// Formats `c` using the gutter color.
    #[inline]
    pub fn as_gutter(&self, c: char) -> Cell {
        Cell::new(c, self.styler.gutter_color)
    }

    /// Formats `c` using a color depending on the current rendering context.
    pub fn as_text(&self, c: char, pos: usize, row: u32, syntax_color: Option<u8>) -> Cell {
        let config = &self.styler.config;

        let fg = if (c == '\n' && config.settings.eol) || c.is_control() {
            config.theme.whitespace_fg
        } else if let Some(fg) = syntax_color {
            fg
        } else {
            config.theme.text_fg
        };

        let bg = if self.selected.contains(&pos) {
            config.theme.select_bg
        } else if config.settings.spotlight && row == self.cursor.row {
            config.theme.spotlight_bg
        } else {
            config.theme.text_bg
        };

        Cell::new(self.convert_char(c), Color::new(fg, bg))
    }

    /// Possibly converts `c` to an alternate display character.
    #[inline]
    fn convert_char(&self, c: char) -> char {
        match c {
            '\n' => {
                if self.styler.config.settings.eol {
                    Self::EOL_CHAR
                } else {
                    ' '
                }
            }
            '\t' => Self::TAB_CHAR,
            c if c.is_control() => Self::CTRL_CHAR,
            c => c,
        }
    }
}
