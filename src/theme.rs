//! Color theme.
use crate::color::Color;
use std::rc::Rc;

pub struct Theme {
    // Colors
    pub text_color: Color,
    pub select_color: Color,
    pub banner_color: Color,
    pub echo_color: Color,
    pub prompt_color: Color,
    pub highlight_color: Color,
    pub number_color: Color,

    // Features
    pub highlighting: bool,
    pub numbering: bool,
}

pub type ThemeRef = Rc<Theme>;

impl Theme {
    const TEXT_FG: u8 = 250;
    const TEXT_BG: u8 = 233;

    const SELECT_FG: u8 = 250;
    const SELECT_BG: u8 = 21;

    const BANNER_FG: u8 = 232;
    const BANNER_BG: u8 = 28;

    const ECHO_FG: u8 = 243;
    const ECHO_BG: u8 = 233;

    const PROMPT_FG: u8 = 243;
    const PROMPT_BG: u8 = 233;

    const HIGHLIGHT_FG: u8 = 250;
    const HIGHLIGHT_BG: u8 = 235;

    const NUMBER_FG: u8 = 34;
    const NUMBER_BG: u8 = 235;

    pub fn new() -> Theme {
        Theme {
            text_color: Color::new(Self::TEXT_FG, Self::TEXT_BG),
            select_color: Color::new(Self::SELECT_FG, Self::SELECT_BG),
            banner_color: Color::new(Self::BANNER_FG, Self::BANNER_BG),
            echo_color: Color::new(Self::ECHO_FG, Self::ECHO_BG),
            prompt_color: Color::new(Self::PROMPT_FG, Self::PROMPT_BG),
            highlight_color: Color::new(Self::HIGHLIGHT_FG, Self::HIGHLIGHT_BG),
            number_color: Color::new(Self::NUMBER_FG, Self::NUMBER_BG),
            highlighting: true,
            numbering: true,
        }
    }

    pub fn to_ref(self) -> ThemeRef {
        Rc::new(self)
    }
}

impl Default for Theme {
    fn default() -> Theme {
        Theme::new()
    }
}
