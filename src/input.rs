//! Input reader.
use crate::display::{Display, Point};
use crate::key::Key;

pub struct Input {
    origin: Point,
    cols: u32,
    prompt: String,
    buffer: Vec<char>,
    cursor: u32,
    display: Display,
}

impl Input {
    pub fn new(origin: Point, cols: u32, prompt: String) -> Input {
        Input {
            origin,
            cols,
            prompt,
            buffer: Vec::new(),
            cursor: 0,
            display: Display::new(origin),
        }
    }

    pub fn process_key(&mut self, key: &Key) {}
}
