//! A collection of functions for reading and writing files to and from buffers.

use crate::buffer::Buffer;
use crate::error::{Error, Result};
use crate::sys::AsString;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::time::SystemTime;

/// Suggested capacity of internal buffers for readers and writers.
const BUFFER_SIZE: usize = 65_536;

/// Opens the file at `path` and reads the contents into `buf`, returning the
/// number of bytes read.
pub fn read_file<P: AsRef<Path>>(path: P, buf: &mut Buffer) -> Result<usize> {
    // Use file size as somewhat imprecise hint to minimize buffer reallocations,
    // particularly when large files are read.
    buf.make_available(get_size(&path)? as usize);
    let file = open_file(&path)?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    buf.read(&mut reader).map_err(|e| to_error(e, path))
}

/// Creates a new file at `path` and writes the contents of `buf`, returning the
/// number of bytes written.
pub fn write_file<P: AsRef<Path>>(path: P, buf: &Buffer) -> Result<usize> {
    let file = create_file(&path)?;
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);
    buf.write(&mut writer).map_err(|e| to_error(e, path))
}

/// Opens the file at `path` for reading.
pub fn open_file<P: AsRef<Path>>(path: P) -> Result<File> {
    let path = path.as_ref();
    File::open(path).map_err(|e| to_error(e, path))
}

/// Creates a new file at `path` for writing.
pub fn create_file<P: AsRef<Path>>(path: P) -> Result<File> {
    let path = path.as_ref();
    File::create(path).map_err(|e| to_error(e, path))
}

/// Returns the size of `path` in bytes.
pub fn get_size<P: AsRef<Path>>(path: P) -> Result<u64> {
    let path = path.as_ref();
    fs::metadata(path)
        .map_err(|e| to_error(e, path))
        .map(|meta| meta.len())
}

/// Returns the [modification timestamp](fs::Metadata::modified) of `path`.
pub fn get_time<P: AsRef<Path>>(path: P) -> Result<SystemTime> {
    let path = path.as_ref();
    fs::metadata(path)
        .map_err(|e| to_error(e, path))
        .and_then(|info| info.modified().map_err(|e| to_error(e, path)))
}

/// Converts an I/O error into its corresponding `Error` adorned with `path`.
fn to_error<P: AsRef<Path>>(e: io::Error, path: P) -> Error {
    Error::io(&path.as_ref().as_string(), e)
}
