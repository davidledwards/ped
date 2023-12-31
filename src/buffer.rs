//! Gap buffer.

use crate::error::{Error, Result};
use std::alloc::{self, Layout};
use std::mem;
use std::ptr;
use std::slice;

#[derive(Debug)]
pub struct Buffer {
    buf: *mut char,
    capacity: usize,
    size: usize,
    gap_start: usize,
    gap_end: usize,
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
            gap_start: 0,
            gap_end: n,
        })
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn forward(&self) -> Forward<'_> {
        Forward { buffer: &self, pos: 0 }
    }

    pub fn forward_at(&self, pos: usize) -> Forward<'_> {
        assert!(pos <= self.size);
        Forward { buffer: &self, pos }
    }

    pub fn backward(&self) -> Backward<'_> {
        Backward { buffer: &self, pos: self.size }
    }

    pub fn backward_at(&self, pos: usize) -> Backward<'_> {
        assert!(pos <= self.size);
        Backward { buffer: &self, pos }
    }

    pub fn insert(&mut self, pos: usize, c: char) -> Result<usize> {
        assert!(pos <= self.size);
        self.ensure(1)?;
        self.align(pos);
        unsafe {
            *self.ptr_of(self.gap_start) = c;
        }
        self.gap_start += 1;
        self.size += 1;
        Ok(pos + 1)
    }

    pub fn insert_chars(&mut self, pos: usize, cs: &Vec<char>) -> Result<usize> {
        assert!(pos <= self.size);
        let n = cs.len();
        self.ensure(n)?;
        self.align(pos);
        unsafe {
            ptr::copy_nonoverlapping(cs.as_ptr(), self.ptr_of(self.gap_start), n);
        }
        self.gap_start += n;
        self.size += n;
        Ok(pos + n)
    }

    pub fn delete(&mut self, pos: usize) -> char {
        assert!(pos < self.size);
        self.align(pos);
        let c = unsafe {
            *self.ptr_of(self.gap_end)
        };
        self.gap_end += 1;
        self.size -= 1;
        c
    }

    pub fn delete_chars(&mut self, start_pos: usize, end_pos: usize) -> Vec<char> {
        assert!(start_pos < self.size);
        assert!(end_pos <= self.size);
        let (start, end) = if start_pos > end_pos {
            (end_pos, start_pos)
        } else {
            (start_pos, end_pos)
        };
        self.align(start);
        let n = end - start;
        let cs = unsafe {
            Vec::from(slice::from_raw_parts(self.ptr_of(self.gap_end), n))
        };
        self.gap_end += n;
        self.size -= n;
        cs
    }

    fn index_of(&self, pos: usize) -> usize {
        if pos < self.gap_start {
            pos
        } else {
            self.gap_end + (pos - self.gap_start)
        }
    }

    fn ptr_of(&self, i: usize) -> *mut char {
        unsafe { self.buf.add(i) }
    }

    fn char_at(&self, pos: usize) -> char {
        unsafe { *self.ptr_of(self.index_of(pos)) }
    }

    fn align(&mut self, pos: usize) {
        if pos < self.gap_start {
            let n = self.gap_start - pos;
            unsafe {
                ptr::copy(self.ptr_of(pos), self.ptr_of(self.gap_end - n), n);
            }
            self.gap_start -= n;
            self.gap_end -= n;
        } else if pos > self.gap_start {
            let n = pos - self.gap_start;
            unsafe {
                ptr::copy(self.ptr_of(self.gap_end), self.ptr_of(self.gap_start), n);
            }
            self.gap_start += n;
            self.gap_end += n;
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
        let gap_end = self.gap_end + (capacity - self.capacity);
        unsafe {
            // Copy sections of original buffer left and right of gap into new buffer.
            ptr::copy_nonoverlapping(self.buf, buf, self.gap_start);
            ptr::copy_nonoverlapping(
                self.buf.add(self.gap_end),
                buf.add(gap_end),
                capacity - gap_end,
            );
        }

        Buffer::dealloc(self.buf, self.capacity);
        self.buf = buf;
        self.capacity = capacity;
        self.gap_end = gap_end;
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
                println!("alloc: {:?}, {}", buf, capacity);
                Ok(buf)
            }
        }
    }

    fn dealloc(buf: *mut char, capacity: usize) {
        let layout = Layout::array::<char>(capacity).unwrap();
        unsafe { alloc::dealloc(buf as *mut u8, layout) }
        println!("dealloc: {:?}, {}", buf, capacity);
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        Buffer::dealloc(self.buf, self.capacity);
    }
}
