//! Color theme.
use std::rc::Rc;

use crate::color::Color;

pub struct Theme {
    pub text_color: Color,
    pub banner_color: Color,
    pub alert_color: Color,
}

pub type ThemeRef = Rc<Theme>;

impl Theme {
    const TEXT_FG: u8 = 15;
    const TEXT_BG: u8 = 233;

    const BANNER_FG: u8 = 232;
    const BANNER_BG: u8 = 253;

    const ALERT_FG: u8 = 2;
    const ALERT_BG: u8 = 232;

    pub fn new() -> Theme {
        Theme {
            text_color: Color::new(Self::TEXT_FG, Self::TEXT_BG),
            banner_color: Color::new(Self::BANNER_FG, Self::BANNER_BG),
            alert_color: Color::new(Self::ALERT_FG, Self::ALERT_BG),
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
