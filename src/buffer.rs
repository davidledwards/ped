//! Gap buffer.

use crate::error::Result;
use std::alloc::{self, Layout};
use std::cmp;
use std::io::{BufRead, Write};
use std::ptr::NonNull;

#[derive(Debug)]
pub struct Buffer {
    buf: NonNull<char>,
    capacity: usize,
    size: usize,
    gap: usize,
    gap_len: usize,
}

// Buffer capacity increments and bounds.
const INIT_CAPACITY: usize = 65_536;
const GROW_CAPACITY: usize = 65_536;
const MAX_CAPACITY: usize = 2_147_483_648;

impl Buffer {
    pub fn new() -> Buffer {
        Buffer::with_capacity(INIT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Buffer {
        let n = if capacity > 0 {
            capacity
        } else {
            INIT_CAPACITY
        };
        Buffer {
            buf: Buffer::alloc(n),
            capacity: n,
            size: 0,
            gap: 0,
            gap_len: n,
        }
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
                NonNull::copy_to(
                    self.buf.add(pos),
                    self.buf.add(self.gap + self.gap_len - n),
                    n,
                );
            }
        } else if pos > self.gap {
            let n = pos - self.gap;
            unsafe {
                NonNull::copy_to(
                    self.buf.add(self.gap + self.gap_len),
                    self.buf.add(self.gap),
                    n,
                );
            }
        }
        self.gap = pos;
        self.gap
    }

    pub fn get_char(&self, pos: usize) -> Option<char> {
        if pos < self.size {
            Some(self.get_char_unchecked(pos))
        } else {
            None
        }
    }

    fn get_char_unchecked(&self, pos: usize) -> char {
        self.read_char(self.index_of(pos))
    }

    pub fn insert_char(&mut self, c: char) -> usize {
        self.ensure(1);
        self.write_char(self.gap, c);
        self.gap += 1;
        self.gap_len -= 1;
        self.size += 1;
        self.gap
    }

    pub fn insert_chars(&mut self, cs: &Vec<char>) -> usize {
        let n = cs.len();
        self.ensure(n);
        unsafe {
            let cs_ptr = NonNull::new_unchecked(cs.as_ptr() as *mut char);
            cs_ptr.copy_to_nonoverlapping(self.ptr_at(self.gap), n);
        }
        self.gap += n;
        self.gap_len -= n;
        self.size += n;
        self.gap
    }

    pub fn remove_char(&mut self) -> Option<char> {
        if self.gap < self.size {
            let c = self.read_char(self.gap + self.gap_len);
            self.gap_len += 1;
            self.size -= 1;
            Some(c)
        } else {
            None
        }
    }

    pub fn remove_chars(&mut self, count: usize) -> Option<Vec<char>> {
        if self.gap < self.size {
            let end = self.gap + self.gap_len;
            let n = cmp::min(count, self.capacity - end);
            let cs =
                Vec::from(unsafe { NonNull::slice_from_raw_parts(self.ptr_at(end), n).as_ref() });
            self.gap_len += n;
            self.size -= n;
            Some(cs)
        } else {
            None
        }
    }

    /// Returns the position of the first character of the line relative to `pos`.
    ///
    /// Specifically, this function returns the position of the character following the
    /// first `\n` encountered when scanning backwards from `pos`, or returns `0` if the
    /// beginning of buffer is reached.
    ///
    /// Note that when scanning backwards, `pos` is an _exclusive_ bound.
    pub fn find_beg_line(&self, pos: usize) -> usize {
        self.backward(pos)
            .index()
            .find(|&(_, c)| c == '\n')
            .map(|(_pos, _)| _pos + 1)
            .unwrap_or(0)
    }

    /// Returns the position of the end of line relative to `pos`.
    ///
    /// Specifically, this function returns the position of the first `\n` encountered when
    /// scanning foewards from `pos`, or returns the end of buffer position if reached first.
    ///
    /// Note that when scanning forwards, `pos` is an _inclusive_ bound.
    pub fn find_end_line(&self, pos: usize) -> usize {
        self.forward(pos)
            .index()
            .find(|&(_, c)| c == '\n')
            .map(|(_pos, _)| _pos)
            .unwrap_or(self.size)
    }

    /// Returns either the position of the end of line relative to `pos` or `n` characters
    /// forward, whichever comes first.
    ///
    /// This function behaves similar to [`find_end_line`](Self::find_end_line), but stops
    /// searching if `n` characters are scanned before `\n` is encountered, essentially
    /// placing a bound on the search range.
    pub fn find_end_line_or(&self, pos: usize, n: usize) -> usize {
        self.forward(pos)
            .index()
            .take(n)
            .find(|&(_, c)| c == '\n')
            .map(|(_pos, _)| _pos)
            .unwrap_or_else(|| cmp::min(pos + n, self.size))
    }

    /// Returns a `Some` containing either the position of the first character of the line
    /// that follows the current line referenced by `pos` or `n` characters forward,
    /// otherwise `None` if end of buffer is reached.
    pub fn find_next_line_or(&self, pos: usize, n: usize) -> Option<usize> {
        self.forward(pos)
            .index()
            .take(n)
            .find(|&(_, c)| c == '\n')
            .map(|(_pos, _)| _pos + 1)
            .or_else(|| (pos + n < self.size).then(|| pos + n))
    }

    pub fn find_prev(&self, pos: usize) -> Option<usize> {
        // find end of previous line first, then find beginning of previous line
        self.backward(pos)
            .index()
            .find(|&(_, c)| c == '\n')
            .and_then(|(_pos, _)| {
                self.backward(_pos)
                    .index()
                    .find(|&(_, c)| c == '\n')
                    .map(|(_pos, _)| _pos + 1)
                    .or(Some(0))
            })
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
                let _ = self.insert_chars(&cs);
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
            let c = self.get_char_unchecked(pos);
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

    pub fn forward(&self, pos: usize) -> Forward<'_> {
        Forward {
            buffer: &self,
            pos: cmp::min(pos, self.size),
        }
    }

    pub fn backward(&self, pos: usize) -> Backward<'_> {
        Backward {
            buffer: &self,
            pos: cmp::min(pos, self.size),
        }
    }

    #[inline(always)]
    fn ptr_at(&self, n: usize) -> NonNull<char> {
        unsafe { self.buf.add(n) }
    }

    #[inline(always)]
    fn index_of(&self, pos: usize) -> usize {
        if pos < self.gap {
            pos
        } else {
            pos + self.gap_len
        }
    }

    #[inline(always)]
    fn read_char(&self, n: usize) -> char {
        unsafe { self.ptr_at(n).read() }
    }

    #[inline(always)]
    fn write_char(&mut self, n: usize, c: char) {
        unsafe { self.ptr_at(n).write(c) }
    }

    /// Ensure that buffer capacity is at least `n` bytes.
    fn ensure(&mut self, n: usize) {
        let free = self.capacity - self.size;
        if n > free {
            self.grow(n - free)
        }
    }

    /// Increase buffer capacity by at least `need` bytes.
    fn grow(&mut self, need: usize) {
        let capacity = if need > MAX_CAPACITY {
            panic!("incremental allocation too large: {} bytes", need);
        } else {
            // This calculation is safe from panic since capacity is always <= MAX_CAPACITY
            // and addition would never overflow because result is sufficiently smaller than
            // usize::MAX.
            (self.capacity + need + GROW_CAPACITY - 1) / GROW_CAPACITY * GROW_CAPACITY
        };

        // Allocate new buffer and copy contents of old buffer.
        let buf = Buffer::alloc(capacity);
        let gap_len = self.gap_len + (capacity - self.capacity);
        unsafe {
            // Copy left of gap.
            NonNull::copy_to_nonoverlapping(self.ptr_at(0), buf, self.gap);

            // Copy right of gap.
            NonNull::copy_to_nonoverlapping(
                self.ptr_at(self.gap + self.gap_len),
                buf.add(self.gap + gap_len),
                capacity - (self.gap + gap_len),
            );
        }

        // Safe to deallocate old buffer and update state to reflect new capacity.
        Buffer::dealloc(self.buf, self.capacity);
        self.buf = buf;
        self.capacity = capacity;
        self.gap_len = gap_len;
    }

    fn alloc(capacity: usize) -> NonNull<char> {
        if capacity > MAX_CAPACITY {
            panic!("allocation too large: {} bytes", capacity);
        }
        let layout = Layout::array::<char>(capacity).unwrap();
        let ptr = unsafe { alloc::alloc(layout) as *mut char };
        NonNull::new(ptr).unwrap_or_else(|| alloc::handle_alloc_error(layout))
    }

    fn dealloc(buf: NonNull<char>, capacity: usize) {
        let layout = Layout::array::<char>(capacity).unwrap();
        unsafe { alloc::dealloc(buf.as_ptr() as *mut u8, layout) }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        Buffer::dealloc(self.buf, self.capacity);
    }
}

pub struct Forward<'a> {
    buffer: &'a Buffer,
    pos: usize,
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
            let c = self.buffer.get_char_unchecked(self.pos);
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }
}

pub struct ForwardIndex<'a> {
    it: Forward<'a>,
}

impl Iterator for ForwardIndex<'_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<(usize, char)> {
        self.it.next().map(|c| (self.it.pos - 1, c))
    }
}

pub struct Backward<'a> {
    buffer: &'a Buffer,
    pos: usize,
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
            let c = self.buffer.get_char_unchecked(self.pos);
            Some(c)
        } else {
            None
        }
    }
}

pub struct BackwardIndex<'a> {
    it: Backward<'a>,
}

impl Iterator for BackwardIndex<'_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<(usize, char)> {
        self.it.next().map(|c| (self.it.pos, c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::iter::{self, zip};

    #[test]
    fn new_buffer() {
        let buf = Buffer::new();
        assert_eq!(buf.capacity, INIT_CAPACITY);
        assert_eq!(buf.size, 0);
        assert_eq!(buf.gap, 0);
        assert_eq!(buf.gap_len, buf.capacity);
    }

    #[test]
    fn new_buffer_with_capacity() {
        const CAP: usize = 17;

        let buf = Buffer::with_capacity(CAP);
        assert_eq!(buf.capacity, CAP);
        assert_eq!(buf.size, 0);
        assert_eq!(buf.gap, 0);
        assert_eq!(buf.gap_len, CAP);
    }

    #[test]
    fn grow_buffer() {
        const CAP: usize = 17;

        let mut buf = Buffer::with_capacity(CAP);
        for c in iter::repeat('*').take(CAP + 1) {
            buf.insert_char(c);
        }
        assert_eq!(buf.capacity, GROW_CAPACITY);
        assert_eq!(buf.size, CAP + 1);
    }

    #[test]
    fn insert() {
        let mut buf = Buffer::new();
        let pos = buf.insert_char('a');
        assert_eq!(pos, 1);
        assert_eq!(buf.get_char(0), Some('a'));
        assert_eq!(buf.size(), 1);

        let pos = buf.insert_char('b');
        assert_eq!(pos, 2);
        assert_eq!(buf.get_char(1), Some('b'));
        assert_eq!(buf.size(), 2);

        let pos = buf.set_pos(1);
        assert_eq!(pos, 1);
        let pos = buf.insert_char('c');
        assert_eq!(pos, 2);
        assert_eq!(buf.get_char(0), Some('a'));
        assert_eq!(buf.get_char(1), Some('c'));
        assert_eq!(buf.get_char(2), Some('b'));
        assert_eq!(buf.size(), 3);
    }

    #[test]
    fn insert_chars() {
        let mut buf = Buffer::new();
        let pos = buf.insert_chars(&vec!['a', 'b', 'c']);
        assert_eq!(pos, 3);
        assert_eq!(buf.get_char(0), Some('a'));
        assert_eq!(buf.get_char(1), Some('b'));
        assert_eq!(buf.get_char(2), Some('c'));
        assert_eq!(buf.size(), 3);

        let pos = buf.set_pos(1);
        assert_eq!(pos, 1);
        let pos = buf.insert_chars(&vec!['d', 'e', 'f']);
        assert_eq!(pos, 4);
        assert_eq!(buf.get_char(0), Some('a'));
        assert_eq!(buf.get_char(1), Some('d'));
        assert_eq!(buf.get_char(2), Some('e'));
        assert_eq!(buf.get_char(3), Some('f'));
        assert_eq!(buf.get_char(4), Some('b'));
        assert_eq!(buf.get_char(5), Some('c'));
        assert_eq!(buf.size(), 6);
    }

    #[test]
    fn delete() {
        const TEXT: &str = "abcdef";

        let mut buf = Buffer::new();
        let cs = TEXT.chars().collect();
        let _ = buf.insert_chars(&cs);
        assert_eq!(buf.size(), cs.len());

        let pos = buf.set_pos(1);
        assert_eq!(pos, 1);
        let c = buf.remove_char();
        assert_eq!(c, Some('b'));
        assert_eq!(buf.get_char(1), Some('c'));
        assert_eq!(buf.size(), cs.len() - 1);
    }

    #[test]
    fn delete_chars() {
        const TEXT: &str = "abcxyzdef";

        let mut buf = Buffer::new();
        let text = TEXT.chars().collect();
        let _ = buf.insert_chars(&text);
        assert_eq!(buf.size(), text.len());

        let pos = buf.set_pos(3);
        assert_eq!(pos, 3);
        let cs = buf.remove_chars(3);
        assert_eq!(cs, Some(vec!['x', 'y', 'z']));
        assert_eq!(buf.get_char(3), Some('d'));
        assert_eq!(buf.size(), text.len() - 3);
    }

    #[test]
    fn read_into_buffer() {
        const TEXT: &str = "ƿŠɎĊȹ·ĽĖ]ɄɁɈǍȶĸĔȚì.İĈËĩ·øǮƩŒƆŉȡȅǫĈǞǿDǶǳȦǧž¬Ǿ3ÙģDíĎȪƐŖUƝËǻ";

        let mut reader = Cursor::new(TEXT.to_string());
        let mut buf = Buffer::new();

        let n = buf.read(&mut reader).unwrap();
        assert_eq!(n, TEXT.chars().count());

        for (a, b) in zip(buf.forward(0), TEXT.chars()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn write_from_buffer() {
        const TEXT: &str = "ųų!)EÝ×vĶǑǟ²ȋØWÚųțòWůĪĎɎ«ƿǎǓC±ţOƹǅĠ/9ŷŌȈïĚſ°ǼȎ¢2^ÁǑī0ÄgŐĢśŧ¶";

        let mut buf = Buffer::new();
        let _ = buf.insert_chars(&TEXT.chars().collect());
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

        let mut buf = Buffer::new();
        assert_eq!(buf.forward(0).next(), None);

        let cs = TEXT.chars().collect();
        let n = buf.insert_chars(&cs);
        assert_eq!(cs.len(), n);

        for (a, b) in zip(buf.forward(0), cs.iter()) {
            assert_eq!(a, *b);
        }

        let pos = buf.set_pos(buf.size() / 2);
        for (a, b) in zip(buf.forward(buf.get_pos()), cs[pos..].iter()) {
            assert_eq!(a, *b);
        }
    }

    #[test]
    fn forward_with_index() {
        const TEXT: &str = "Lorem ipsum dolor sit amet, consectetur porttitor";

        let mut buf = Buffer::new();
        let cs = TEXT.chars().collect();
        let _ = buf.insert_chars(&cs);

        for ((a_pos, a), (b_pos, b)) in zip(buf.forward(0).index(), zip(0..cs.len(), cs)) {
            assert_eq!(a_pos, b_pos);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn backward() {
        const TEXT: &str = "Lorem ipsum dolor sit amet, consectetur porttitor";

        let mut buf = Buffer::new();
        assert_eq!(buf.backward(buf.size()).next(), None);

        let cs = TEXT.chars().collect();
        let n = buf.insert_chars(&cs);
        assert_eq!(cs.len(), n);

        for (a, b) in zip(buf.backward(buf.size()), cs.iter().rev()) {
            assert_eq!(a, *b);
        }

        let pos = buf.set_pos(buf.size() / 2);
        for (a, b) in zip(buf.backward(buf.get_pos()), cs[0..pos].iter().rev()) {
            assert_eq!(a, *b);
        }
    }

    #[test]
    fn backward_with_index() {
        const TEXT: &str = "Lorem ipsum dolor sit amet, consectetur porttitor";

        let mut buf = Buffer::new();
        let cs = TEXT.chars().collect();
        let _ = buf.insert_chars(&cs);

        for ((a_pos, a), (b_pos, b)) in zip(
            buf.backward(buf.size()).index(),
            zip((0..cs.len()).rev(), cs.into_iter().rev()),
        ) {
            assert_eq!(a_pos, b_pos);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn find_beg_line() {
        const TEXT: &str = "abc\ndef\nghi";

        let mut buf = Buffer::new();
        let cs = TEXT.chars().collect();
        let _ = buf.insert_chars(&cs);

        // All chars in `def\n` range should find the same beginning of line.
        for pos in 4..8 {
            let p = buf.find_beg_line(pos);
            assert_eq!(p, 4);
        }

        // All chars in `abc\n` range should find the same beginning of line, which
        // also happens to be beginning of buffer.
        for pos in 0..4 {
            let p = buf.find_beg_line(pos);
            assert_eq!(p, 0);
        }
    }
}
