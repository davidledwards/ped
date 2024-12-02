//! A collection of functions related to help.

#![allow(unused_must_use)]

use crate::buffer::Buffer;
use crate::editor::{Editor, EditorRef};
use crate::key::KEY_MAPPINGS;
use crate::{BUILD_DATE, BUILD_HASH, PACKAGE_NAME, PACKAGE_VERSION};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

pub const HELP_EDITOR_NAME: &str = "@help";
pub const KEYS_EDITOR_NAME: &str = "@keys";
pub const BINDINGS_EDITOR_NAME: &str = "@bindings";

pub fn help_editor() -> EditorRef {
    Editor::transient(HELP_EDITOR_NAME, Some(help_buffer())).to_ref()
}

fn help_buffer() -> Buffer {
    let mut out = String::new();
    write!(out, include_str!("../include/help-header.in"));
    writeln!(
        out,
        "\nBuild: {PACKAGE_NAME} {PACKAGE_VERSION} ({BUILD_HASH} {BUILD_DATE})\n"
    );
    write!(out, include_str!("../include/help-keys.in"));
    make_buffer(&out)
}

pub fn keys_editor() -> EditorRef {
    Editor::transient(KEYS_EDITOR_NAME, Some(keys_buffer())).to_ref()
}

pub fn keys_content() -> String {
    let keys = prepare_keys();
    let mut out = String::new();
    for key_name in keys {
        writeln!(out, "{key_name}");
    }
    out
}

fn keys_buffer() -> Buffer {
    const HEADER: &str = "Keys";

    let keys = prepare_keys();
    let mut out = String::new();
    writeln!(out, "{HEADER}");
    writeln!(out, "{:-<1$}", "", HEADER.len());
    for key_name in keys {
        writeln!(out, "{key_name}");
    }
    make_buffer(&out)
}

fn prepare_keys() -> Vec<String> {
    let mut keys = KEY_MAPPINGS
        .iter()
        .map(|(key_name, _)| key_name.to_string())
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

pub fn bindings_editor(bindings: &HashMap<String, String>) -> EditorRef {
    Editor::transient(BINDINGS_EDITOR_NAME, Some(bindings_buffer(bindings))).to_ref()
}

pub fn bindings_content(bindings: &HashMap<String, String>) -> String {
    let bindings = prepare_bindings(&bindings);
    let mut out = String::new();
    for (key_seq, op) in bindings {
        writeln!(out, "\"{key_seq}\", \"{op}\"");
    }
    out
}

fn bindings_buffer(bindings: &HashMap<String, String>) -> Buffer {
    const HEADER_KEY_SEQ: &str = "Key Sequence";
    const HEADER_OP: &str = "Operation";

    // Prettify and sort key sequences.
    let bindings = prepare_bindings(&bindings);

    // Calculate maximum width of key sequences to align output.
    let key_width = bindings.keys().fold(HEADER_KEY_SEQ.len(), |width, k| {
        if k.len() > width {
            k.len()
        } else {
            width
        }
    });

    // Emit formatted bindings.
    let mut out = String::new();
    writeln!(out, "{:<key_width$}   {}", HEADER_KEY_SEQ, HEADER_OP);
    writeln!(
        out,
        "{:<key_width$}   {}",
        "_".repeat(HEADER_KEY_SEQ.len()),
        "_".repeat(HEADER_OP.len())
    );
    for (key_seq, op) in bindings {
        writeln!(out, "{key_seq:<key_width$}   {op}");
    }
    make_buffer(&out)
}

fn prepare_bindings(bindings: &HashMap<String, String>) -> BTreeMap<String, String> {
    bindings
        .iter()
        .map(|(key_seq, op)| (pretty_keys(key_seq).join(" "), op.to_string()))
        .collect::<BTreeMap<_, _>>()
}

/// Returns a vector of individual key names extracted from `key_seq`.
///
/// A sequence beginning with `ESC` `<key>` is replaced with `M-<key>`.
fn pretty_keys(key_seq: &str) -> Vec<String> {
    let keys = key_seq
        .split(':')
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    match keys.get(0) {
        Some(key) if key == "ESC" => {
            if let Some(next_key) = keys.get(1) {
                let mut alt_keys = vec![format!("M-{next_key}")];
                for key in keys.iter().skip(2) {
                    alt_keys.push(key.clone());
                }
                alt_keys
            } else {
                keys
            }
        }
        _ => keys,
    }
}

/// Returns a buffer containing the contents of `out`.
fn make_buffer(out: &str) -> Buffer {
    let mut buffer = Buffer::new();
    buffer.insert_str(out);
    buffer.set_pos(0);
    buffer
}
