//! I/O operations with buffers.
use crate::buffer::Buffer;
use crate::error::{Error, Result};
use crate::sys::AsString;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::time::SystemTime;

/// Suggested capacity of internal buffers for readers and writers.
const BUFFER_SIZE: usize = 65_536;

pub fn read_file(path: &str, buf: &mut Buffer) -> Result<usize> {
    let file = open_file(path)?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    buf.read(&mut reader)
        .map_err(|e| to_error(e, path.as_ref()))
}

pub fn write_file(path: &str, buf: &Buffer) -> Result<usize> {
    let file = create_file(path)?;
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);
    buf.write(&mut writer)
        .map_err(|e| to_error(e, path.as_ref()))
}

pub fn open_file(path: &str) -> Result<File> {
    File::open(path).map_err(|e| to_error(e, path))
}

pub fn create_file(path: &str) -> Result<File> {
    File::create(path).map_err(|e| to_error(e, path))
}

pub fn get_time(path: &str) -> Result<SystemTime> {
    fs::metadata(path)
        .map_err(|e| to_error(e, path))
        .and_then(|info| info.modified().map_err(|e| to_error(e, path)))
}

fn to_error(e: io::Error, path: &str) -> Error {
    Error::io(Some(&device_of(path)), e)
}

fn device_of(path: &str) -> String {
    Path::new(path).as_string()
}
