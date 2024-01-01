//! Gap buffer.

use crate::error::{Error, Result};
use std::alloc::{self, Layout};
use std::cmp;
use std::mem;
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

pub struct Backward<'a> {
    buffer: &'a Buffer,
    pos: usize,
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

impl Buffer {
    const INIT_CAPACITY: usize = 65536;
    const GROW_CAPACITY: usize = 65536;
    const MAX_CAPACITY: usize = isize::MAX as usize / mem::size_of::<char>();

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

    pub fn forward_iter(&self, pos: usize) -> Forward<'_> {
        Forward {
            buffer: &self,
            pos: cmp::min(pos, self.size),
        }
    }

    pub fn backward_iter(&self, pos: usize) -> Backward<'_> {
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
        // New capacity rounds up to next increment while satisfying need requested by caller.
        let capacity = self
            .capacity
            .saturating_add(need)
            .saturating_add(Buffer::GROW_CAPACITY - 1)
            .saturating_div(Buffer::GROW_CAPACITY)
            .saturating_mul(Buffer::GROW_CAPACITY);

        let buf = Buffer::alloc(capacity)?;
        let gap_len = self.gap_len + (capacity - self.capacity);
        unsafe {
            // Copy sections of original buffer left and right of gap into new buffer.
            ptr::copy_nonoverlapping(self.buf, buf, self.gap);
            ptr::copy_nonoverlapping(
                self.buf.add(self.gap + self.gap_len),
                buf.add(self.gap + gap_len),
                capacity - (self.gap + gap_len),
            );
        }

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

impl Drop for Buffer {
    fn drop(&mut self) {
        Buffer::dealloc(self.buf, self.capacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter;

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
}
