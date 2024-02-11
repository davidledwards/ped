//! I/O operations with buffers.

use crate::buffer::Buffer;
use crate::error::Result;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

// Suggested capacity of internal buffers for readers and writers.
const BUFFER_SIZE: usize = 65_536;

pub fn read_file<P>(path: P, buf: &mut Buffer) -> Result<usize>
where
    P: AsRef<Path>,
{
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, File::open(path)?);
    buf.read(&mut reader)
}

pub fn write_file<P>(path: P, buf: &Buffer) -> Result<usize>
where
    P: AsRef<Path>,
{
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, File::create(path)?);
    buf.write(&mut writer)
}
