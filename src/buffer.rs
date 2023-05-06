use crate::error::{Error, Result};
use std::alloc::{self, Layout};
use std::mem;
use std::ptr;

#[derive(Debug)]
pub struct Buffer {
    buf: *mut char,
    capacity: usize,
    gap_start: usize,
    gap_end: usize,
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
            gap_start: 0,
            gap_end: n,
        })
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn size(&self) -> usize {
        self.capacity - (self.gap_end - self.gap_start)
    }

    fn start_ptr(&mut self) -> *mut char {
        unsafe { self.buf.add(self.gap_start) }
    }

    fn end_ptr(&mut self) -> *mut char {
        unsafe { self.buf.add(self.gap_end) }
    }

    fn gap(&self) -> usize {
        self.gap_end - self.gap_start
    }

    pub fn insert(&mut self, c: char) -> Result<()> {
        self.ensure_space(1)?;
        unsafe { *self.start_ptr() = c };
        self.gap_start += 1;
        Ok(())
    }

    pub fn insert_str(&mut self, text: &str) -> Result<()> {
        for c in text.chars() {
            self.insert(c)?;
        }
        Ok(())
    }

    pub fn remove(&mut self) {
        if self.gap_start > 0 {
            self.gap_start -= 1;
        }
    }

    fn ensure_space(&mut self, n: usize) -> Result<()> {
        if n > self.gap() {
            self.grow(n - self.gap())
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
            ptr::copy_nonoverlapping(buf, self.buf, self.gap_start);
            ptr::copy_nonoverlapping(
                buf.add(gap_end),
                self.buf.add(self.gap_end),
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
