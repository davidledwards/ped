//! A collection of functions related to help.

// Necessary to disable warnings from infallible uses of write!() that do not
// check return values.
#![allow(unused_must_use)]

use crate::buffer::Buffer;
use crate::config::{ConfigurationRef, Theme};
use crate::editor::{Editor, EditorRef};
use crate::etc;
use crate::key::{self, Key, KEY_MAPPINGS};
use crate::op::OP_MAPPINGS;
use crate::source::Source;
use indexmap::IndexMap;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

pub const HELP_EDITOR_NAME: &str = "help";
pub const KEYS_EDITOR_NAME: &str = "keys";
pub const OPS_EDITOR_NAME: &str = "operations";
pub const BINDINGS_EDITOR_NAME: &str = "bindings";
pub const COLORS_EDITOR_NAME: &str = "colors";

/// Returns an ephemeral editor, named `@help`, containing general help content.
pub fn help_editor(config: ConfigurationRef) -> EditorRef {
    Editor::readonly(
        config,
        Source::as_ephemeral(HELP_EDITOR_NAME),
        help_buffer(),
    )
    .to_ref()
}

fn help_buffer() -> Buffer {
    let mut buf = Buffer::new();
    writeln!(buf, include_str!("include/help-header.in"));
    writeln!(buf, "[Build]");
    writeln!(buf, "{}\n", etc::version());
    write!(buf, include_str!("include/help-keys.in"));
    buf.set_pos(0);
    buf
}

/// Returns an ephemeral editor, named `@keys`, containing a list of available keys.
pub fn keys_editor(config: ConfigurationRef) -> EditorRef {
    Editor::readonly(
        config,
        Source::as_ephemeral(KEYS_EDITOR_NAME),
        keys_buffer(),
    )
    .to_ref()
}

/// Returns a formatted list of available keys.
pub fn keys_content() -> String {
    let keys = prepare_keys();
    let mut out = String::new();
    for key_name in keys {
        writeln!(out, "{key_name}");
    }
    out
}

fn keys_buffer() -> Buffer {
    const HEADER: &str = "[Keys]";

    let keys = prepare_keys();
    let mut buf = Buffer::new();
    writeln!(buf, "{HEADER}");
    for key_name in keys {
        writeln!(buf, "{key_name}");
    }
    buf.set_pos(0);
    buf
}

fn prepare_keys() -> Vec<String> {
    let mut keys = KEY_MAPPINGS
        .iter()
        .map(|(key_name, _)| key_name.to_string())
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

/// Returns an ephemeral editor, named `@operations`, containing a list of available
/// editing operations.
pub fn ops_editor(config: ConfigurationRef) -> EditorRef {
    Editor::readonly(config, Source::as_ephemeral(OPS_EDITOR_NAME), ops_buffer()).to_ref()
}

/// Returns a formatted list of available editing operations.
pub fn ops_content() -> String {
    let ops = prepare_ops();
    let mut out = String::new();
    for op in ops {
        writeln!(out, "{op}");
    }
    out
}

fn ops_buffer() -> Buffer {
    const HEADER: &str = "[Operations]";

    let ops = prepare_ops();
    let mut buf = Buffer::new();
    writeln!(buf, "{HEADER}");
    for op in ops {
        writeln!(buf, "{op}");
    }
    buf.set_pos(0);
    buf
}

fn prepare_ops() -> Vec<String> {
    let mut ops = OP_MAPPINGS
        .iter()
        .map(|(op, _)| op.to_string())
        .collect::<Vec<_>>();
    ops.sort();
    ops
}

/// Returns an ephemeral editor, named `@bindings`, containing a list of key bindings.
pub fn bindings_editor(config: ConfigurationRef) -> EditorRef {
    let buffer = bindings_buffer(config.bindings.bindings());
    Editor::readonly(config, Source::as_ephemeral(BINDINGS_EDITOR_NAME), buffer).to_ref()
}

/// Returns a TOML-formatted list of key bindings.
pub fn bindings_content(bindings: &HashMap<Vec<Key>, String>) -> String {
    let bindings = prepare_bindings(bindings);
    let mut out = String::new();
    for (key_seq, op) in bindings {
        let key_seq = key_seq.replace(' ', ":");
        writeln!(out, "\"{key_seq}\" = \"{op}\"");
    }
    out
}

fn bindings_buffer(bindings: &HashMap<Vec<Key>, String>) -> Buffer {
    const HEADER_KEY: &str = "[Key]";
    const HEADER_OP: &str = "[Operation]";

    // Prettify and sort key sequences.
    let bindings = prepare_bindings(bindings);

    // Calculate maximum width of key sequences to align output.
    let key_width = bindings.keys().fold(HEADER_KEY.len(), |width, k| {
        if k.len() > width {
            k.len()
        } else {
            width
        }
    });

    // Emit formatted bindings.
    let mut buf = Buffer::new();
    writeln!(buf, "{:<key_width$}   {}", HEADER_KEY, HEADER_OP);
    for (key_seq, op) in bindings {
        writeln!(buf, "{key_seq:<key_width$}   {op}");
    }
    buf.set_pos(0);
    buf
}

fn prepare_bindings(bindings: &HashMap<Vec<Key>, String>) -> BTreeMap<String, String> {
    bindings
        .iter()
        .map(|(keys, op)| (key::pretty(keys), op.to_string()))
        .collect::<BTreeMap<_, _>>()
}

/// Returns an ephemeral editor, named `@colors`, containing a list of color names
/// and values.
pub fn colors_editor(config: ConfigurationRef) -> EditorRef {
    let buffer = colors_buffer(config.colors.colors());
    Editor::readonly(config, Source::as_ephemeral(COLORS_EDITOR_NAME), buffer).to_ref()
}

/// Returns a TOML-formatted list of color names and values.
pub fn colors_content(colors: &HashMap<String, u8>) -> String {
    let colors = prepare_colors(colors);
    let mut out = String::new();
    for (name, color) in colors {
        writeln!(out, "{name} = {color}");
    }
    out
}

fn colors_buffer(colors: &HashMap<String, u8>) -> Buffer {
    const HEADER_NAME: &str = "[Name]";
    const HEADER_VALUE: &str = "[Value]";

    // Calculate maximum width of color names to align output.
    let colors = prepare_colors(colors);
    let name_width = colors.keys().fold(HEADER_NAME.len(), |width, name| {
        if name.len() > width {
            name.len()
        } else {
            width
        }
    });

    // Emit formatted colors.
    let mut buf = Buffer::new();
    writeln!(buf, "{:<name_width$}   {}", HEADER_NAME, HEADER_VALUE);
    for (name, color) in colors {
        writeln!(buf, "{name:<name_width$}   {color}");
    }
    buf.set_pos(0);
    buf
}

fn prepare_colors(colors: &HashMap<String, u8>) -> IndexMap<String, u8> {
    // Sort by color value rather than color name.
    let mut colors = colors.iter().collect::<Vec<_>>();
    colors.sort_by(|a, b| a.1.cmp(b.1));

    // Preserve insertion order such that iteration over resulting map will produce
    // entries whose color values appear sorted.
    colors
        .iter()
        .map(|(name, color)| (name.to_string(), **color))
        .collect::<IndexMap<_, _>>()
}

/// Returns a TOML-formatted list of theme color names and values.
pub fn theme_content(theme: &Theme) -> String {
    const COLORS: [(&str, fn(&Theme) -> u8); 13] = [
        ("text-fg", |t| t.text_fg),
        ("text-bg", |t| t.text_bg),
        ("select-bg", |t| t.select_bg),
        ("spotlight-bg", |t| t.spotlight_bg),
        ("whitespace-fg", |t| t.whitespace_fg),
        ("accent-fg", |t| t.accent_fg),
        ("echo-fg", |t| t.echo_fg),
        ("prompt-fg", |t| t.prompt_fg),
        ("banner-fg", |t| t.banner_fg),
        ("active-bg", |t| t.active_bg),
        ("inactive-bg", |t| t.inactive_bg),
        ("margin-fg", |t| t.margin_fg),
        ("margin-bg", |t| t.margin_bg),
    ];

    let mut out = String::new();
    for (name, t_fn) in COLORS {
        writeln!(out, "{name} = {}", t_fn(theme));
    }
    out
}
