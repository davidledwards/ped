//! Gap buffer.

use libc::PF_NS;

use crate::error::{Error, Result};
use std::alloc::{self, Layout};
use std::cmp;
use std::io::{BufRead, Write};
use std::ptr;
use std::slice;

#[derive(Debug)]
pub struct Buffer {
    buf: *mut char,
    capacity: usize,
    size: usize,
    gap: usize,
    gap_len: usize,
}

pub struct Forward<'a> {
    buffer: &'a Buffer,
    pos: usize,
}

pub struct ForwardIndex<'a> {
    it: Forward<'a>,
}

pub struct Backward<'a> {
    buffer: &'a Buffer,
    pos: usize,
}

pub struct BackwardIndex<'a> {
    it: Backward<'a>,
}

impl Buffer {
    // Initial capacity of buffer if not specified.
    const INIT_CAPACITY: usize = 65_536;

    // Smallest increment of capacity growth.
    const GROW_CAPACITY: usize = 65_536;

    // Largest possible buffer capacity.
    const MAX_CAPACITY: usize = 2_147_483_648;

    pub fn new() -> Result<Buffer> {
        Buffer::with_capacity(Buffer::INIT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Result<Buffer> {
        let n = if capacity > 0 {
            capacity
        } else {
            Buffer::INIT_CAPACITY
        };
        Ok(Buffer {
            buf: Buffer::alloc(n)?,
            capacity: n,
            size: 0,
            gap: 0,
            gap_len: n,
        })
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn get_pos(&self) -> usize {
        self.gap
    }

    pub fn set_pos(&mut self, pos: usize) -> usize {
        let pos = cmp::min(pos, self.size);
        if pos < self.gap {
            let n = self.gap - pos;
            unsafe {
                ptr::copy(
                    self.buf.add(pos),
                    self.buf.add(self.gap + self.gap_len - n),
                    n,
                );
            }
        } else if pos > self.gap {
            let n = pos - self.gap;
            unsafe {
                ptr::copy(
                    self.buf.add(self.gap + self.gap_len),
                    self.buf.add(self.gap),
                    n,
                );
            }
        }
        self.gap = pos;
        self.gap
    }

    pub fn get(&self, pos: usize) -> Option<char> {
        if pos < self.size {
            Some(self.char_at(pos))
        } else {
            None
        }
    }

    pub fn insert(&mut self, c: char) -> Result<usize> {
        self.ensure(1)?;
        unsafe {
            *self.buf.add(self.gap) = c;
        }
        self.gap += 1;
        self.gap_len -= 1;
        self.size += 1;
        Ok(self.gap)
    }

    pub fn insert_chars(&mut self, cs: &Vec<char>) -> Result<usize> {
        let n = cs.len();
        self.ensure(n)?;
        unsafe {
            ptr::copy_nonoverlapping(cs.as_ptr(), self.buf.add(self.gap), n);
        }
        self.gap += n;
        self.gap_len -= n;
        self.size += n;
        Ok(self.gap)
    }

    pub fn delete(&mut self) -> Option<char> {
        if self.gap < self.size {
            let c = unsafe { *self.buf.add(self.gap + self.gap_len) };
            self.gap_len += 1;
            self.size -= 1;
            Some(c)
        } else {
            None
        }
    }

    pub fn delete_chars(&mut self, count: usize) -> Option<Vec<char>> {
        if self.gap < self.size {
            let end = self.gap + self.gap_len;
            let n = cmp::min(count, self.capacity - end);
            let cs = unsafe { Vec::from(slice::from_raw_parts(self.buf.add(end), n)) };
            self.gap_len += n;
            self.size -= n;
            Some(cs)
        } else {
            None
        }
    }

    // find beginning of line relative to pos or beginning of buffer
    pub fn find_bol(&self, pos: usize) -> usize {
        // Scan backwards to find first \n or beginning of buffer, whichever comes first,
        // denoting beginning of line.
        let r = self.backward_from(pos).index().find(|&(_, c)| c == '\n');

        // Found position is always pointing to \n, but we want next character.
        match r {
            Some((_pos, _)) => _pos + 1,
            None => 0,
        }
    }

    // find end of line relative to pos or end of buffer
    // points to \n, otherwise end of buffer
    pub fn find_eol(&self, pos: usize) -> usize {
        let r = self.forward_from(pos).index().find(|&(_, c)| c == '\n');
        match r {
            Some((_pos, _)) => _pos,
            None => self.size,
        }
    }

    // find end of line relative to pos but only n distance away from pos, whichever
    // comes first
    // resulting pos could be \n, end of buffer, or arbitrary char if n is reached first
    pub fn find_eol_or(&self, pos: usize, n: usize) -> usize {
        let r = self
            .forward_from(pos)
            .index()
            .take(n)
            .find(|&(_, c)| c == '\n');
        match r {
            Some((_pos, _)) => _pos,
            None => cmp::min(pos + n, self.size),
        }
    }

    pub fn find_next_or(&self, pos: usize, n: usize) -> Option<usize> {
        // Scans forward until \n encountered, but not to exceed specified number of
        // characters.
        let r = self
            .forward_from(pos)
            .index()
            .take(n)
            .find(|&(_, c)| c == '\n');

        // If find operation terminates before end of buffer or maximum number of
        // characters are scanned, this implies \n is found, so skip to next character.
        // Otherwise, distinguish between both conditions that could cause find to
        // terminate early.
        match r {
            Some((_pos, _)) => Some(_pos + 1),
            None => {
                if pos + n < self.size {
                    Some(pos + n)
                } else {
                    None
                }
            }
        }
    }

    pub fn find_prev(&self, pos: usize) -> Option<usize> {
        // find end of previous line first
        let r = self.backward_from(pos).index().find(|&(_, c)| c == '\n');
        match r {
            Some((_pos, _)) => {
                // then find beginning of previous line
                let r = self.backward_from(_pos).index().find(|&(_, c)| c == '\n');
                match r {
                    Some((_pos, _)) => Some(_pos + 1),
                    None => Some(0),
                }
            }
            None => None,
        }
    }

    pub fn read<R>(&mut self, reader: &mut R) -> Result<usize>
    where
        R: BufRead,
    {
        // Approximate number of characters to decode from reader before inserting into buffer.
        const READ_CHUNK_SIZE: usize = 16_384;

        let mut chunk = String::with_capacity(READ_CHUNK_SIZE);
        let mut count = 0;

        loop {
            let n = reader.read_line(&mut chunk)?;
            if (n > 0 && chunk.len() >= READ_CHUNK_SIZE) || n == 0 {
                // Inserts chunk into buffer when either condition occurs:
                // - enough characters have been read to reach trigger, or
                // - reader has reached EOF
                let cs = chunk.chars().collect();
                let _ = self.insert_chars(&cs)?;
                count += cs.len();
                chunk.clear();
            }
            if n == 0 {
                break;
            }
        }
        Ok(count)
    }

    pub fn write<W>(&self, writer: &mut W) -> Result<usize>
    where
        W: Write,
    {
        // Approximate number of bytes to encode from buffer before sending to writer.
        const WRITE_CHUNK_SIZE: usize = 65_536;

        let mut bytes = [0; 4];
        let mut chunk = Vec::with_capacity(WRITE_CHUNK_SIZE);
        let mut count = 0;

        for pos in 0..self.size {
            let c = self.char_at(pos);
            let encoding = c.encode_utf8(&mut bytes);
            chunk.extend_from_slice(encoding.as_bytes());
            if chunk.len() >= WRITE_CHUNK_SIZE || pos == self.size - 1 {
                // Sends chunk of encoded characters to writer when either condition occurs:
                // - enough bytes have been encoded to reach trigger, or
                // - end of buffer
                let _ = writer.write_all(chunk.as_slice())?;
                count += chunk.len();
                chunk.clear();
            }
        }
        Ok(count)
    }

    pub fn forward(&self) -> Forward<'_> {
        Forward {
            buffer: &self,
            pos: self.get_pos(),
        }
    }

    pub fn forward_from(&self, pos: usize) -> Forward<'_> {
        Forward {
            buffer: &self,
            pos: cmp::min(pos, self.size),
        }
    }

    pub fn backward(&self) -> Backward<'_> {
        Backward {
            buffer: &self,
            pos: self.get_pos(),
        }
    }

    pub fn backward_from(&self, pos: usize) -> Backward<'_> {
        Backward {
            buffer: &self,
            pos: cmp::min(pos, self.size),
        }
    }

    fn char_at(&self, pos: usize) -> char {
        unsafe { *self.buf.add(self.index_of(pos)) }
    }

    fn index_of(&self, pos: usize) -> usize {
        if pos < self.gap {
            pos
        } else {
            pos + self.gap_len
        }
    }

    fn ensure(&mut self, n: usize) -> Result<()> {
        let free = self.capacity - self.size;
        if n > free {
            self.grow(n - free)
        } else {
            Ok(())
        }
    }

    fn grow(&mut self, need: usize) -> Result<()> {
        // New capacity rounds up to next increment while satisfying need.
        let capacity = self
            .capacity
            .saturating_add(need)
            .saturating_add(Buffer::GROW_CAPACITY - 1)
            .saturating_div(Buffer::GROW_CAPACITY)
            .saturating_mul(Buffer::GROW_CAPACITY);

        // Allocate new buffer and copy contents of old buffer.
        let buf = Buffer::alloc(capacity)?;
        let gap_len = self.gap_len + (capacity - self.capacity);
        unsafe {
            // Copy left of gap.
            ptr::copy_nonoverlapping(self.buf, buf, self.gap);

            // Copy right of gap.
            ptr::copy_nonoverlapping(
                self.buf.add(self.gap + self.gap_len),
                buf.add(self.gap + gap_len),
                capacity - (self.gap + gap_len),
            );
        }

        // Safe to deallocate old buffer and update state to reflect new capacity.
        Buffer::dealloc(self.buf, self.capacity);
        self.buf = buf;
        self.capacity = capacity;
        self.gap_len = gap_len;
        Ok(())
    }

    fn alloc(capacity: usize) -> Result<*mut char> {
        if capacity > Buffer::MAX_CAPACITY {
            Err(Error::BufferTooLarge(capacity))
        } else {
            let layout = Layout::array::<char>(capacity).unwrap();
            let buf = unsafe { alloc::alloc(layout) as *mut char };
            if buf.is_null() {
                Err(Error::OutOfMemory)
            } else {
                Ok(buf)
            }
        }
    }

    fn dealloc(buf: *mut char, capacity: usize) {
        let layout = Layout::array::<char>(capacity).unwrap();
        unsafe { alloc::dealloc(buf as *mut u8, layout) }
    }
}

impl<'a> Forward<'a> {
    pub fn index(self) -> ForwardIndex<'a> {
        ForwardIndex { it: self }
    }
}

impl Iterator for Forward<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        if self.pos < self.buffer.size {
            let c = self.buffer.char_at(self.pos);
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }
}

impl Iterator for ForwardIndex<'_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<(usize, char)> {
        match self.it.next() {
            Some(c) => Some((self.it.pos - 1, c)),
            None => None,
        }
    }
}

impl<'a> Backward<'a> {
    pub fn index(self) -> BackwardIndex<'a> {
        BackwardIndex { it: self }
    }
}

impl Iterator for Backward<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        if self.pos > 0 {
            self.pos -= 1;
            let c = self.buffer.char_at(self.pos);
            Some(c)
        } else {
            None
        }
    }
}

impl Iterator for BackwardIndex<'_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<(usize, char)> {
        match self.it.next() {
            Some(c) => Some((self.it.pos, c)),
            None => None,
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        Buffer::dealloc(self.buf, self.capacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::iter::{self, zip};

    #[test]
    fn new_buffer() {
        let buf = Buffer::new().unwrap();
        assert!(!buf.buf.is_null());
        assert_eq!(buf.capacity, Buffer::INIT_CAPACITY);
        assert_eq!(buf.size, 0);
        assert_eq!(buf.gap, 0);
        assert_eq!(buf.gap_len, buf.capacity);
    }

    #[test]
    fn new_buffer_with_capacity() {
        const CAP: usize = 17;

        let buf = Buffer::with_capacity(CAP).unwrap();
        assert!(!buf.buf.is_null());
        assert_eq!(buf.capacity, CAP);
        assert_eq!(buf.size, 0);
        assert_eq!(buf.gap, 0);
        assert_eq!(buf.gap_len, CAP);
    }

    #[test]
    fn grow_buffer() {
        const CAP: usize = 17;

        let mut buf = Buffer::with_capacity(CAP).unwrap();
        for c in iter::repeat('*').take(CAP + 1) {
            buf.insert(c).unwrap();
        }
        assert_eq!(buf.capacity, Buffer::GROW_CAPACITY);
        assert_eq!(buf.size, CAP + 1);
    }

    #[test]
    fn insert() {
        let mut buf = Buffer::new().unwrap();
        let pos = buf.insert('a').unwrap();
        assert_eq!(pos, 1);
        assert_eq!(buf.get(0), Some('a'));
        assert_eq!(buf.size(), 1);

        let pos = buf.insert('b').unwrap();
        assert_eq!(pos, 2);
        assert_eq!(buf.get(1), Some('b'));
        assert_eq!(buf.size(), 2);

        let pos = buf.set_pos(1);
        assert_eq!(pos, 1);
        let pos = buf.insert('c').unwrap();
        assert_eq!(pos, 2);
        assert_eq!(buf.get(0), Some('a'));
        assert_eq!(buf.get(1), Some('c'));
        assert_eq!(buf.get(2), Some('b'));
        assert_eq!(buf.size(), 3);
    }

    #[test]
    fn insert_chars() {
        let mut buf = Buffer::new().unwrap();
        let pos = buf.insert_chars(&vec!['a', 'b', 'c']).unwrap();
        assert_eq!(pos, 3);
        assert_eq!(buf.get(0), Some('a'));
        assert_eq!(buf.get(1), Some('b'));
        assert_eq!(buf.get(2), Some('c'));
        assert_eq!(buf.size(), 3);

        let pos = buf.set_pos(1);
        assert_eq!(pos, 1);
        let pos = buf.insert_chars(&vec!['d', 'e', 'f']).unwrap();
        assert_eq!(pos, 4);
        assert_eq!(buf.get(0), Some('a'));
        assert_eq!(buf.get(1), Some('d'));
        assert_eq!(buf.get(2), Some('e'));
        assert_eq!(buf.get(3), Some('f'));
        assert_eq!(buf.get(4), Some('b'));
        assert_eq!(buf.get(5), Some('c'));
        assert_eq!(buf.size(), 6);
    }

    #[test]
    fn delete() {
        const TEXT: &str = "abcdef";

        let mut buf = Buffer::new().unwrap();
        let cs = TEXT.chars().collect();
        let _ = buf.insert_chars(&cs).unwrap();
        assert_eq!(buf.size(), cs.len());

        let pos = buf.set_pos(1);
        assert_eq!(pos, 1);
        let c = buf.delete();
        assert_eq!(c, Some('b'));
        assert_eq!(buf.get(1), Some('c'));
        assert_eq!(buf.size(), cs.len() - 1);
    }

    #[test]
    fn delete_chars() {
        const TEXT: &str = "abcxyzdef";

        let mut buf = Buffer::new().unwrap();
        let text = TEXT.chars().collect();
        let _ = buf.insert_chars(&text).unwrap();
        assert_eq!(buf.size(), text.len());

        let pos = buf.set_pos(3);
        assert_eq!(pos, 3);
        let cs = buf.delete_chars(3);
        assert_eq!(cs, Some(vec!['x', 'y', 'z']));
        assert_eq!(buf.get(3), Some('d'));
        assert_eq!(buf.size(), text.len() - 3);
    }

    #[test]
    fn read_into_buffer() {
        const TEXT: &str = "ƿŠɎĊȹ·ĽĖ]ɄɁɈǍȶĸĔȚì.İĈËĩ·øǮƩŒƆŉȡȅǫĈǞǿDǶǳȦǧž¬Ǿ3ÙģDíĎȪƐŖUƝËǻ";

        let mut reader = Cursor::new(TEXT.to_string());
        let mut buf = Buffer::new().unwrap();

        let n = buf.read(&mut reader).unwrap();
        assert_eq!(n, TEXT.chars().count());

        for (a, b) in zip(buf.forward_from(0), TEXT.chars()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn write_from_buffer() {
        const TEXT: &str = "ųų!)EÝ×vĶǑǟ²ȋØWÚųțòWůĪĎɎ«ƿǎǓC±ţOƹǅĠ/9ŷŌȈïĚſ°ǼȎ¢2^ÁǑī0ÄgŐĢśŧ¶";

        let mut buf = Buffer::new().unwrap();
        let _ = buf.insert_chars(&TEXT.chars().collect()).unwrap();
        let mut writer = Cursor::new(Vec::new());

        let n = buf.write(&mut writer).unwrap();
        assert_eq!(n, TEXT.len());

        for (a, b) in zip(writer.into_inner(), TEXT.bytes()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn forward() {
        const TEXT: &str = "Lorem ipsum dolor sit amet, consectetur porttitor";

        let mut buf = Buffer::new().unwrap();
        assert_eq!(buf.forward_from(0).next(), None);

        let cs = TEXT.chars().collect();
        let n = buf.insert_chars(&cs).unwrap();
        assert_eq!(cs.len(), n);

        for (a, b) in zip(buf.forward_from(0), cs.iter()) {
            assert_eq!(a, *b);
        }

        let pos = buf.set_pos(buf.size() / 2);
        for (a, b) in zip(buf.forward(), cs[pos..].iter()) {
            assert_eq!(a, *b);
        }
    }

    #[test]
    fn forward_with_index() {
        const TEXT: &str = "Lorem ipsum dolor sit amet, consectetur porttitor";

        let mut buf = Buffer::new().unwrap();
        let cs = TEXT.chars().collect();
        let _ = buf.insert_chars(&cs).unwrap();

        for ((a_pos, a), (b_pos, b)) in zip(buf.forward_from(0).index(), zip(0..cs.len(), cs)) {
            assert_eq!(a_pos, b_pos);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn backward() {
        const TEXT: &str = "Lorem ipsum dolor sit amet, consectetur porttitor";

        let mut buf = Buffer::new().unwrap();
        assert_eq!(buf.backward_from(buf.size()).next(), None);

        let cs = TEXT.chars().collect();
        let n = buf.insert_chars(&cs).unwrap();
        assert_eq!(cs.len(), n);

        for (a, b) in zip(buf.backward_from(buf.size()), cs.iter().rev()) {
            assert_eq!(a, *b);
        }

        let pos = buf.set_pos(buf.size() / 2);
        for (a, b) in zip(buf.backward(), cs[0..pos].iter().rev()) {
            assert_eq!(a, *b);
        }
    }

    #[test]
    fn backward_with_index() {
        const TEXT: &str = "Lorem ipsum dolor sit amet, consectetur porttitor";

        let mut buf = Buffer::new().unwrap();
        let cs = TEXT.chars().collect();
        let _ = buf.insert_chars(&cs).unwrap();

        for ((a_pos, a), (b_pos, b)) in zip(
            buf.backward_from(buf.size()).index(),
            zip((0..cs.len()).rev(), cs.into_iter().rev()),
        ) {
            assert_eq!(a_pos, b_pos);
            assert_eq!(a, b);
        }
    }
}
