//! I/O operations with buffers.

use crate::buffer::Buffer;
use crate::error::{Error, Result};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

// Suggested capacity of internal buffers for readers and writers.
const BUFFER_SIZE: usize = 65_536;

pub fn read_file<P>(path: P, buf: &mut Buffer) -> Result<usize>
where
    P: AsRef<Path>,
{
    let file = File::open(&path).map_err(|e| Error::file(&path_string(&path), e))?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    buf.read(&mut reader)
}

pub fn write_file<P>(path: P, buf: &Buffer) -> Result<usize>
where
    P: AsRef<Path>,
{
    let file = File::create(&path).map_err(|e| Error::file(&path_string(&path), e))?;
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);
    buf.write(&mut writer)
}

fn path_string<T>(path: &T) -> String
where
    T: AsRef<Path>,
{
    path.as_ref().to_string_lossy().to_string()
}
