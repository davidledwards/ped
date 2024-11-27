//! # I/O operations
use crate::buffer::Buffer;
use crate::error::{Error, Result};
use crate::sys::AsString;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::time::SystemTime;

/// Suggested capacity of internal buffers for readers and writers.
const BUFFER_SIZE: usize = 65_536;

pub fn read_file<P: AsRef<Path>>(path: P, buf: &mut Buffer) -> Result<usize> {
    let path = path.as_ref();
    let file = open_file(path)?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    buf.read(&mut reader).map_err(|e| to_error(e, path))
}

pub fn write_file<P: AsRef<Path>>(path: P, buf: &Buffer) -> Result<usize> {
    let path = path.as_ref();
    let file = create_file(path)?;
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);
    buf.write(&mut writer).map_err(|e| to_error(e, path))
}

pub fn open_file<P: AsRef<Path>>(path: P) -> Result<File> {
    File::open(path.as_ref()).map_err(|e| to_error(e, path))
}

pub fn create_file<P: AsRef<Path>>(path: P) -> Result<File> {
    File::create(path.as_ref()).map_err(|e| to_error(e, path))
}

pub fn get_time<P: AsRef<Path>>(path: P) -> Result<SystemTime> {
    let path = path.as_ref();
    fs::metadata(path)
        .map_err(|e| to_error(e, path))
        .and_then(|info| info.modified().map_err(|e| to_error(e, path)))
}

fn to_error<P: AsRef<Path>>(e: io::Error, path: P) -> Error {
    Error::io(Some(&device_of(path)), e)
}

fn device_of<P: AsRef<Path>>(path: P) -> String {
    path.as_ref().as_string()
}
