//! Color theme.
use std::rc::Rc;

use crate::color::Color;

pub struct Theme {
    pub text_color: Color,
    pub select_color: Color,
    pub banner_color: Color,
    pub echo_color: Color,
    pub prompt_color: Color,
}

pub type ThemeRef = Rc<Theme>;

impl Theme {
    const TEXT_FG: u8 = 15;
    const TEXT_BG: u8 = 233;

    const SELECT_FG: u8 = 233;
    const SELECT_BG: u8 = 15;

    const BANNER_FG: u8 = 232;
    const BANNER_BG: u8 = 40;

    const ECHO_FG: u8 = 2;
    const ECHO_BG: u8 = 233;

    const PROMPT_FG: u8 = 40;
    const PROMPT_BG: u8 = 233;

    pub fn new() -> Theme {
        Theme {
            text_color: Color::new(Self::TEXT_FG, Self::TEXT_BG),
            select_color: Color::new(Self::SELECT_FG, Self::SELECT_BG),
            banner_color: Color::new(Self::BANNER_FG, Self::BANNER_BG),
            echo_color: Color::new(Self::ECHO_FG, Self::ECHO_BG),
            prompt_color: Color::new(Self::PROMPT_FG, Self::PROMPT_BG),
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
