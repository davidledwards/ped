//! Contains everything related to colors.
//!
//! Colors follow the ANSI 8-bit standard, which is referenced
//! [here](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit).

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::result;

/// A combination of _foreground_ and _background_ color.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Color {
    pub fg: u8,
    pub bg: u8,
}

/// A collection of color mappings.
pub struct Colors {
    color_map: HashMap<String, u8>,
}

/// A color value representing either a _number_ in the range of `0` to `255`, or as a
/// _string_ that refers to a named color.
pub enum ColorValue {
    Number(u8),
    Name(String),
}

/// A deserialization visitor that accepts either color values as numbers in the range
/// of `0` to `255`, or as strings representing either named colors or values that can
/// be parsed as numbers.
pub struct ColorVisitor;

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

impl Display for ColorValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ColorValue::Number(value) => write!(f, "{value}"),
            ColorValue::Name(name) => write!(f, "{name}"),
        }
    }
}

impl Colors {
    pub fn apply(&mut self, colors: &HashMap<String, u8>) {
        for (name, color) in colors {
            self.color_map.insert(name.to_string(), *color);
        }
    }

    pub fn lookup(&self, name: &str) -> Option<u8> {
        self.color_map
            .get(name)
            .map(|color| *color)
            .or_else(|| name.parse::<u8>().ok())
    }

    pub fn lookup_value(&self, value: &ColorValue) -> Option<u8> {
        match value {
            ColorValue::Name(name) => self.lookup(name),
            ColorValue::Number(color) => Some(*color),
        }
    }

    pub fn colors(&self) -> &HashMap<String, u8> {
        &self.color_map
    }

    /// Returns a mapping of standard color names to color values.
    fn init_color_map() -> HashMap<String, u8> {
        let mut color_map = HashMap::new();
        for (name, color) in Self::COLOR_MAPPINGS {
            color_map.insert(name.to_string(), color);
        }
        color_map
    }

    /// Predefined mapping of ANSI standard and extended colors.
    const COLOR_MAPPINGS: [(&str, u8); 34] = [
        // --- ANSI colors ---
        ("black", 0),
        ("red", 1),
        ("green", 2),
        ("yellow", 3),
        ("blue", 4),
        ("magenta", 5),
        ("cyan", 6),
        ("white", 7),
        ("gray", 8),
        ("bright-red", 9),
        ("bright-green", 10),
        ("bright-yellow", 11),
        ("bright-blue", 12),
        ("bright-magenta", 13),
        ("bright-cyan", 14),
        ("bright-white", 15),
        // --- custom colors ---
        ("pale-green", 30),
        ("charcoal-mist", 60),
        ("slate-frost", 61),
        ("deep-sapphire", 69),
        ("verdant-spring", 79),
        ("crimson-ember", 88),
        ("sage-green", 102),
        ("misty-aqua", 109),
        ("crystal-cyan", 111),
        ("twilight-blue", 117),
        ("lavender-haze", 136),
        ("seafoam-mist", 147),
        ("burnt-orange", 166),
        ("orchid-bloom", 175),
        ("amber-dusk", 180),
        ("copper-flame", 208),
        ("peachy-orange", 216),
        ("golden-glow", 223),
    ];
}

impl Default for Colors {
    fn default() -> Colors {
        Colors {
            color_map: Self::init_color_map(),
        }
    }
}

impl<'a> Deserialize<'a> for ColorValue {
    fn deserialize<T: Deserializer<'a>>(deser: T) -> result::Result<ColorValue, T::Error> {
        deser.deserialize_any(ColorVisitor)
    }
}

impl ColorVisitor {
    const EXPECT_MSG: &str = "a number in the range of [0, 255] or a string";
    const ERROR_MSG: &str = "color values must be in the range of [0, 255]";
}

impl<'a> Visitor<'a> for ColorVisitor {
    type Value = ColorValue;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", Self::EXPECT_MSG)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<ColorValue, E> {
        if value >= 0 && value <= 255 {
            Ok(ColorValue::Number(value as u8))
        } else {
            Err(de::Error::custom(Self::ERROR_MSG))
        }
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<ColorValue, E> {
        if value <= 255 {
            Ok(ColorValue::Number(value as u8))
        } else {
            Err(de::Error::custom(Self::ERROR_MSG))
        }
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<ColorValue, E> {
        Ok(ColorValue::Name(value.to_string()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<ColorValue, E> {
        Ok(ColorValue::Name(value))
    }
}
