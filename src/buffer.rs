//! An implementation of a gap buffer, which is the primary data structure for
//! representing and manipulating text.
//!
//! Details on the gap buffer data structure can be found at
//! <https://en.wikipedia.org/wiki/Gap_buffer>.

use std::alloc::{self, Layout};
use std::cell::RefCell;
use std::cmp;
use std::io::{BufRead, Write};
use std::ops::{ControlFlow, Index};
use std::ptr::NonNull;
use std::rc::Rc;
use std::slice;

#[derive(Debug)]
pub struct Buffer {
    buf: NonNull<char>,
    capacity: usize,
    size: usize,
    gap: usize,
    gap_len: usize,
}

pub type BufferRef = Rc<RefCell<Buffer>>;

pub type Result<T> = std::result::Result<T, std::io::Error>;

impl Buffer {
    const INIT_CAPACITY: usize = 65_536;
    const GROW_CAPACITY: usize = 65_536;
    const MAX_CAPACITY: usize = 2_147_483_648;

    pub fn new() -> Buffer {
        Buffer::with_capacity(Self::INIT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Buffer {
        let n = if capacity > 0 {
            capacity
        } else {
            Self::INIT_CAPACITY
        };
        Buffer {
            buf: Buffer::alloc(n),
            capacity: n,
            size: 0,
            gap: 0,
            gap_len: n,
        }
    }

    /// Turns the buffer into a [`BufferRef`].
    pub fn to_ref(self) -> BufferRef {
        Rc::new(RefCell::new(self))
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    #[inline]
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

    #[allow(dead_code)]
    pub fn get_char(&self, pos: usize) -> Option<char> {
        if pos < self.size {
            Some(*self.get_char_unchecked(pos))
        } else {
            None
        }
    }

    fn get_char_unchecked(&self, pos: usize) -> &char {
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

    pub fn insert_str(&mut self, cs: &str) -> usize {
        for c in cs.chars() {
            self.insert_char(c);
        }
        self.gap
    }

    pub fn insert(&mut self, cs: &[char]) -> usize {
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
            let c = *self.read_char(self.gap + self.gap_len);
            self.gap_len += 1;
            self.size -= 1;
            Some(c)
        } else {
            None
        }
    }

    pub fn remove(&mut self, count: usize) -> Vec<char> {
        if self.gap < self.size {
            let end = self.gap + self.gap_len;
            let n = cmp::min(count, self.capacity - end);
            let cs = unsafe { NonNull::slice_from_raw_parts(self.ptr_at(end), n).as_ref() };
            self.gap_len += n;
            self.size -= n;
            Vec::from(cs)
        } else {
            vec![]
        }
    }

    pub fn copy(&self, from_pos: usize, to_pos: usize) -> Vec<char> {
        if from_pos == to_pos {
            vec![]
        } else {
            let (from_pos, to_pos) = if from_pos < to_pos {
                (from_pos, cmp::min(to_pos, self.size))
            } else {
                (to_pos, cmp::min(from_pos, self.size))
            };
            let count = to_pos - from_pos;
            let mut cs = Vec::with_capacity(count);
            let (from_pos, count) = if from_pos < self.gap {
                let n = cmp::min(to_pos, self.gap) - from_pos;
                unsafe {
                    let s = slice::from_raw_parts(self.buf.add(from_pos).as_ptr(), n);
                    cs.extend_from_slice(s);
                }
                (from_pos + n, count - n)
            } else {
                (from_pos, count)
            };
            if count > 0 {
                let pos = self.index_of(from_pos);
                unsafe {
                    let s = slice::from_raw_parts(self.buf.add(pos).as_ptr(), count);
                    cs.extend_from_slice(s);
                }
            }
            cs
        }
    }

    /// Returns the `0`-based line number corresponding to `pos`.
    pub fn line_of(&self, pos: usize) -> u32 {
        self.forward(0).take(pos).filter(|c| *c == '\n').count() as u32
    }

    /// Returns the position of the first character of the `0`-based `line` number.
    ///
    /// If `line` would extend beyond the end of the buffer, then the end of buffer
    /// is returned.
    pub fn find_line(&self, line: u32) -> usize {
        if line > 0 {
            let r = self.forward(0).index().try_fold(0, |l, (pos, c)| {
                if c == '\n' {
                    let l = l + 1;
                    if l == line {
                        ControlFlow::Break(pos + 1)
                    } else {
                        ControlFlow::Continue(l)
                    }
                } else {
                    ControlFlow::Continue(l)
                }
            });
            match r {
                ControlFlow::Break(pos) => pos,
                _ => self.size,
            }
        } else {
            0
        }
    }

    /// Returns the position of the first character of the line relative to `pos`.
    ///
    /// Specifically, this function returns the position of the character following the
    /// first `\n` encountered when scanning backwards from `pos`, or returns `0` if the
    /// beginning of buffer is reached.
    ///
    /// Note that when scanning backwards, `pos` is an _exclusive_ bound.
    pub fn find_start_line(&self, pos: usize) -> usize {
        self.backward(pos)
            .index()
            .find(|&(_, c)| c == '\n')
            .map(|(_pos, _)| _pos + 1)
            .unwrap_or(0)
    }

    /// Returns a tuple containing the position of the next line relative to `pos` and
    /// whether or not the prior line was terminated with `\n`.
    ///
    /// Specifically, this function returns the position following the first `\n`
    /// encountered when scanning forward from `pos`, or returns the end of buffer
    /// position if reached first. The end-of-buufer scenario is the only condition which
    /// would cause the second tuple value to return `false`.
    ///
    /// Note that when scanning forward, `pos` is an _inclusive_ bound.
    pub fn find_next_line(&self, pos: usize) -> (usize, bool) {
        self.forward(pos)
            .index()
            .find(|&(_, c)| c == '\n')
            .map(|(_pos, _)| (_pos + 1, true))
            .unwrap_or((self.size, false))
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
                let cs = chunk.chars().collect::<Vec<_>>();
                let _ = self.insert(&cs);
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

    pub fn iter(&self) -> Forward<'_> {
        self.forward(0)
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
    fn read_char(&self, n: usize) -> &char {
        unsafe { self.ptr_at(n).as_ref() }
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
        let capacity = if need > Self::MAX_CAPACITY {
            panic!("incremental allocation too large: {} bytes", need);
        } else {
            // This calculation is safe from panic since capacity is always <= MAX_CAPACITY
            // and addition would never overflow because result is sufficiently smaller than
            // usize::MAX.
            (self.capacity + need + Self::GROW_CAPACITY - 1) / Self::GROW_CAPACITY
                * Self::GROW_CAPACITY
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
        if capacity > Self::MAX_CAPACITY {
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

impl Index<usize> for Buffer {
    type Output = char;

    fn index(&self, pos: usize) -> &char {
        self.get_char_unchecked(pos)
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        Buffer::dealloc(self.buf, self.capacity);
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Buffer {
        let buf = Buffer::alloc(self.capacity);
        unsafe {
            NonNull::copy_to_nonoverlapping(self.ptr_at(0), buf, self.capacity);
        }
        Buffer { buf, ..*self }
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
            Some(*c)
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
            Some(*c)
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

impl<'a> IntoIterator for &'a Buffer {
    type Item = char;
    type IntoIter = Forward<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.forward(0)
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
        assert_eq!(buf.capacity, Buffer::INIT_CAPACITY);
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
    fn clone_buffer() {
        const TEXT: &str = "abcdefghij";

        // Insert arbitrary text before cloning since need to compare bitwise values
        // from original buffer.
        let mut buf = Buffer::new();
        buf.insert_str(TEXT);
        let cloned_buf = buf.clone();

        assert_eq!(buf.capacity, cloned_buf.capacity);
        assert_eq!(buf.size, cloned_buf.size);
        assert_eq!(buf.gap, cloned_buf.gap);
        assert_eq!(buf.gap_len, cloned_buf.gap_len);

        unsafe {
            let a = slice::from_raw_parts(buf.buf.as_ptr(), buf.capacity);
            let b = slice::from_raw_parts(cloned_buf.buf.as_ptr(), cloned_buf.capacity);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn grow_buffer() {
        const CAP: usize = 17;

        let mut buf = Buffer::with_capacity(CAP);
        for c in iter::repeat('*').take(CAP + 1) {
            buf.insert_char(c);
        }
        assert_eq!(buf.capacity, Buffer::GROW_CAPACITY);
        assert_eq!(buf.size, CAP + 1);
    }

    #[test]
    fn insert_char() {
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
    fn insert() {
        let mut buf = Buffer::new();
        let pos = buf.insert(&['a', 'b', 'c']);
        assert_eq!(pos, 3);
        assert_eq!(buf.get_char(0), Some('a'));
        assert_eq!(buf.get_char(1), Some('b'));
        assert_eq!(buf.get_char(2), Some('c'));
        assert_eq!(buf.size(), 3);

        let pos = buf.set_pos(1);
        assert_eq!(pos, 1);
        let pos = buf.insert(&['d', 'e', 'f']);
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
    fn remove_char() {
        const TEXT: &str = "abcdef";

        let mut buf = Buffer::new();
        let cs = TEXT.chars().collect::<Vec<_>>();
        let _ = buf.insert(&cs);
        assert_eq!(buf.size(), cs.len());

        let pos = buf.set_pos(1);
        assert_eq!(pos, 1);
        let c = buf.remove_char();
        assert_eq!(c, Some('b'));
        assert_eq!(buf.get_char(1), Some('c'));
        assert_eq!(buf.size(), cs.len() - 1);
    }

    #[test]
    fn remove() {
        const TEXT: &str = "abcxyzdef";

        let mut buf = Buffer::new();
        let text = TEXT.chars().collect::<Vec<_>>();
        let _ = buf.insert(&text);
        assert_eq!(buf.size(), text.len());

        let pos = buf.set_pos(3);
        assert_eq!(pos, 3);
        let cs = buf.remove(3);
        assert_eq!(cs, vec!['x', 'y', 'z']);
        assert_eq!(buf.get_char(3), Some('d'));
        assert_eq!(buf.size(), text.len() - 3);

        buf.set_pos(buf.size());
        assert_eq!(buf.remove(1), vec![]);
        buf.set_pos(0);
        assert_eq!(buf.remove(0), vec![]);
    }

    #[test]
    fn copy() {
        const TEXT: &str = "abcdefghijklmnopqrstuvwxyz";

        let mut buf = Buffer::new();
        let text = TEXT.chars().collect::<Vec<_>>();
        let _ = buf.insert(&text);

        // Test copy range when entirely before gap.
        buf.set_pos(10);
        let cs = buf.copy(2, 7);
        assert_eq!(cs, vec!['c', 'd', 'e', 'f', 'g']);

        // Test copy range when entirely after gap.
        let cs = buf.copy(12, 17);
        assert_eq!(cs, vec!['m', 'n', 'o', 'p', 'q']);

        // Test copy range when straddling gap.
        let cs = buf.copy(6, 15);
        assert_eq!(cs, vec!['g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o']);

        // Test empty copy.
        let cs = buf.copy(10, 10);
        assert_eq!(cs, vec![]);

        // Test copy with range outside of actual size.
        let cs = buf.copy(0, usize::MAX);
        assert_eq!(cs, TEXT.chars().collect::<Vec<_>>());
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
        let _ = buf.insert(&TEXT.chars().collect::<Vec<_>>());
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

        let cs = TEXT.chars().collect::<Vec<_>>();
        let n = buf.insert(&cs);
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
        let cs = TEXT.chars().collect::<Vec<_>>();
        let _ = buf.insert(&cs);

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

        let cs = TEXT.chars().collect::<Vec<_>>();
        let n = buf.insert(&cs);
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
        let cs = TEXT.chars().collect::<Vec<_>>();
        let _ = buf.insert(&cs);

        for ((a_pos, a), (b_pos, b)) in zip(
            buf.backward(buf.size()).index(),
            zip((0..cs.len()).rev(), cs.into_iter().rev()),
        ) {
            assert_eq!(a_pos, b_pos);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn find_start_line() {
        const TEXT: &str = "abc\ndef\nghi";

        let mut buf = Buffer::new();
        let cs = TEXT.chars().collect::<Vec<_>>();
        let _ = buf.insert(&cs);

        // All chars in `def\n` range should find the same beginning of line.
        for pos in 4..8 {
            let p = buf.find_start_line(pos);
            assert_eq!(p, 4);
        }

        // All chars in `abc\n` range should find the same beginning of line, which
        // also happens to be beginning of buffer.
        for pos in 0..4 {
            let p = buf.find_start_line(pos);
            assert_eq!(p, 0);
        }
    }
}
