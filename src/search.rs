//! Text searching.

use crate::buffer::Buffer;
use std::cmp;
use std::collections::HashMap;
use std::ops::Range;

pub struct Pattern {
    pattern: Vec<char>,
    shift: HashMap<char, usize>,
}

impl Pattern {
    pub fn new(pattern: &str) -> Pattern {
        let pattern = pattern.chars().collect::<Vec<_>>();
        let mut shift = HashMap::new();
        for (i, c) in pattern.iter().enumerate() {
            shift.insert(*c, i);
        }
        Pattern { pattern, shift }
    }

    pub fn search(&self, buffer: &Buffer, range: Range<usize>) -> Option<usize> {
        let pat_len = self.pattern.len();
        if pat_len > 0 {
            let mut pos = range.start;
            let end_pos = cmp::min(range.end, buffer.size());
            let stop_pos = end_pos - cmp::min(pat_len, end_pos);
            while pos <= stop_pos {
                let mut pat_pos = pat_len;
                while pat_pos > 0 && self.pattern[pat_pos - 1] == buffer[pos + pat_pos - 1] {
                    pat_pos -= 1;
                }
                if pat_pos == 0 {
                    return Some(pos);
                } else {
                    let shift = self
                        .shift
                        .get(&buffer[pos + pat_pos - 1])
                        .map(|n| pat_pos - 1 - n)
                        .unwrap_or(pat_pos);
                    pos += cmp::max(shift, 1);
                }
            }
            None
        } else {
            None
        }
    }
}
