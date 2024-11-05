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
    let file = open_file(path.as_ref())?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    buf.read(&mut reader)
}

pub fn write_file<P>(path: P, buf: &Buffer) -> Result<usize>
where
    P: AsRef<Path>,
{
    let file = create_file(path.as_ref())?;
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);
    buf.write(&mut writer)
}

fn open_file(path: &Path) -> Result<File> {
    File::open(path).map_err(|e| Error::io(Some(&device_of(path)), e))
}

fn create_file(path: &Path) -> Result<File> {
    File::create(path).map_err(|e| Error::io(Some(&device_of(path)), e))
}

fn device_of(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
