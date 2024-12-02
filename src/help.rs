//! A collection of functions related to help.

use crate::buffer::Buffer;
use crate::{BUILD_DATE, BUILD_HASH, PACKAGE_NAME, PACKAGE_VERSION};

pub const EDITOR_NAME: &str = "@help";

pub fn help() -> Buffer {
    let mut buffer = Buffer::new();
    buffer.insert_str(include_str!("../include/help-header.in"));
    buffer.insert_str(&format!(
        "Build: {PACKAGE_NAME} {PACKAGE_VERSION} ({BUILD_HASH} {BUILD_DATE})\n\n"
    ));
    buffer.insert_str(include_str!("../include/help-keys.in"));
    buffer.set_pos(0);
    buffer
}
