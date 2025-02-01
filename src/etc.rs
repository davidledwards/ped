//! Useful functions.

use std::ops::ControlFlow;

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_HASH: &str = env!("BUILD_HASH");
pub const BUILD_DATE: &str = env!("BUILD_DATE");
pub const SOURCE_URL: &str = env!("SOURCE_URL");

/// Returns version information.
pub fn version() -> String {
    format!(
        "{} {} ({} {})",
        PACKAGE_NAME,
        PACKAGE_VERSION,
        &BUILD_HASH[0..7],
        BUILD_DATE,
    )
}

/// Returns the byte offset in `buf` corresponding to the `pos`-th character, which is
/// guaranteed to be aligned to a UTF-8 code point boundary in `buf`.
///
/// If `buf` contains less than `pos` characters, then `buf.len()` is returned.
pub fn pos_to_offset(buf: &str, pos: usize) -> usize {
    buf.chars()
        .take(pos)
        .fold(0, |offset, c| offset + c.len_utf8())
}

/// Returns the `pos`-th character in `buf` corresponding to the byte `offset`.
///
/// If `buf` contains less than `offset` bytes, then the value returned is equal to
/// `buf.chars().count()`.
///
/// Note that the `offset` _should_ be aligned to a UTF-8 code point boundary in `buf`,
/// otherwise the value returned will be the `pos`-th character occurring before
/// `offset`.
pub fn offset_to_pos(buf: &str, offset: usize) -> usize {
    let result = buf.chars().try_fold((0, 0), |(ofs, pos), c| {
        if ofs < offset {
            ControlFlow::Continue((ofs + c.len_utf8(), pos + 1))
        } else {
            ControlFlow::Break((ofs, pos))
        }
    });
    match result {
        ControlFlow::Break((_, pos)) => pos,
        ControlFlow::Continue((_, pos)) => pos,
    }
}
