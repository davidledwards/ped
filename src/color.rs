//! A representation of color.
//!
//! Colors follow the ANSI 8-bit standard, which is referenced
//! [here](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit).

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;
use std::result;

/// A combination of *foreground* and *background* color.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Color {
    pub fg: u8,
    pub bg: u8,
}

type ColorMap = HashMap<&'static str, u8>;

pub struct Colors {
    color_map: ColorMap,
}

impl Colors {
    pub fn new() -> Colors {
        Colors {
            color_map: init_color_map(),
        }
    }

    pub fn lookup(&self, name: &str) -> Option<u8> {
        self.color_map
            .get(name)
            .map(|color| *color)
            .or_else(|| name.parse::<u8>().ok())
    }
}

const COLOR_MAPPINGS: [(&str, u8); 16] = [
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
];

fn init_color_map() -> ColorMap {
    let mut color_map = ColorMap::new();
    for (name, color) in COLOR_MAPPINGS {
        color_map.insert(name, color);
    }
    color_map
}

//pub struct ColorVisitor;

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

// impl<'a> Deserialize<'a> for Color {
//     fn deserialize<T>(deser: T) -> result::Result<Color, T::Error>
//     where
//         T: Deserializer<'a>,
//     {
//         deser.deserialize_tuple(2, ColorVisitor)
//     }
// }

// impl<'a> Visitor<'a> for ColorVisitor {
//     type Value = Color;

//     fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "expecting `[u8, u8]` for Color")
//     }

//     fn visit_seq<T>(self, mut seq: T) -> result::Result<Color, T::Error>
//     where
//         T: SeqAccess<'a>,
//     {
//         let fg = seq
//             .next_element()?
//             .ok_or_else(|| de::Error::invalid_length(0, &self))?;
//         let bg = seq
//             .next_element()?
//             .ok_or_else(|| de::Error::invalid_length(1, &self))?;
//         Ok(Color::new(fg, bg))
//     }
// }
