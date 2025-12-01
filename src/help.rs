//! A collection of functions related to help.

#![allow(unused_must_use, reason = "infallible calls to write!()")]

use crate::buffer::Buffer;
use crate::config::{ConfigurationRef, Theme};
use crate::editor::{Editor, EditorRef};
use crate::etc;
use crate::key::{self, KEY_MAPPINGS, Key};
use crate::op::{self, OP_MAPPINGS};
use crate::source::Source;
use crate::syntax::Syntax;
use indexmap::IndexMap;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

pub const HELP_EDITOR_NAME: &str = "help";
pub const KEYS_EDITOR_NAME: &str = "keys";
pub const OPS_EDITOR_NAME: &str = "operations";
pub const BINDINGS_EDITOR_NAME: &str = "bindings";
pub const COLORS_EDITOR_NAME: &str = "colors";
pub const SYNTAXES_EDITOR_NAME: &str = "syntaxes";

/// Returns an ephemeral editor, named `@help`, containing general help content.
pub fn help_editor(config: ConfigurationRef) -> EditorRef {
    let buffer = help_buffer(config.bindings.bindings());
    Editor::readonly(config, Source::as_ephemeral(HELP_EDITOR_NAME), buffer).into_ref()
}

fn help_buffer(bindings: &HashMap<Vec<Key>, String>) -> Buffer {
    // Calculate maximum width of key sequences to align output.
    let bindings = prepare_bindings(bindings);
    let key_width = bindings
        .keys()
        .fold(0, |width, k| if k.len() > width { k.len() } else { width });

    // Emit both static and dynamic content.
    let mut buf = Buffer::new();
    write!(buf, include_str!("include/help.in"));
    writeln!(buf, "{}\n", etc::version());
    write!(buf, include_str!("include/help-keys.in"));
    for (key_seq, op) in bindings {
        let desc = op::describe(&op).unwrap_or("");
        writeln!(buf, "{key_seq:<key_width$}   {desc}");
    }
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
    .into_ref()
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
    let keys = prepare_keys();
    let mut buf = Buffer::new();
    writeln!(buf, "[Key]");
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
    Editor::readonly(config, Source::as_ephemeral(OPS_EDITOR_NAME), ops_buffer()).into_ref()
}

/// Returns a formatted list of available editing operations.
pub fn ops_content() -> String {
    let ops = prepare_ops();
    let mut out = String::new();
    for (op, _) in ops {
        writeln!(out, "{op}");
    }
    out
}

fn ops_buffer() -> Buffer {
    const HEADER_OP: &str = "[Operation]";
    const HEADER_DESC: &str = "[Description]";

    // Calculate maximum width of key sequences to align output.
    let ops = prepare_ops();
    let key_width = ops.keys().map(|k| k.len()).max().unwrap_or(HEADER_OP.len());

    // Emit formatted operations.
    let mut buf = Buffer::new();
    writeln!(buf, "{HEADER_OP:<key_width$}   {HEADER_DESC}");
    for (op, desc) in ops {
        writeln!(buf, "{op:<key_width$}   {desc}");
    }
    buf.set_pos(0);
    buf
}

fn prepare_ops() -> BTreeMap<String, String> {
    OP_MAPPINGS
        .iter()
        .map(|(op, _, desc)| (op.to_string(), desc.to_string()))
        .collect()
}

/// Returns an ephemeral editor, named `@bindings`, containing a list of key bindings.
pub fn bindings_editor(config: ConfigurationRef) -> EditorRef {
    let buffer = bindings_buffer(config.bindings.bindings());
    Editor::readonly(config, Source::as_ephemeral(BINDINGS_EDITOR_NAME), buffer).into_ref()
}

/// Returns a TOML-formatted list of key bindings.
pub fn bindings_content(bindings: &HashMap<Vec<Key>, String>) -> String {
    let bindings = prepare_bindings(bindings);
    let mut out = String::new();
    for (key_seq, op) in bindings {
        let key_seq = key_seq.replace(' ', ":");
        writeln!(out, "'{key_seq}' = '{op}'");
    }
    out
}

fn bindings_buffer(bindings: &HashMap<Vec<Key>, String>) -> Buffer {
    const HEADER_KEY: &str = "[Key]";
    const HEADER_OP: &str = "[Operation]";

    // Prettify and sort key sequences.
    let bindings = prepare_bindings(bindings);

    // Calculate maximum width of key sequences to align output.
    let key_width = bindings
        .keys()
        .map(|k| k.len())
        .max()
        .unwrap_or(HEADER_KEY.len());

    // Emit formatted bindings.
    let mut buf = Buffer::new();
    writeln!(buf, "{HEADER_KEY:<key_width$}   {HEADER_OP}");
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
        .collect()
}

/// Returns an ephemeral editor, named `@colors`, containing a list of color names
/// and values.
pub fn colors_editor(config: ConfigurationRef) -> EditorRef {
    let buffer = colors_buffer(config.colors.colors());
    Editor::readonly(config, Source::as_ephemeral(COLORS_EDITOR_NAME), buffer).into_ref()
}

/// Returns a TOML-formatted list of color names and values.
pub fn colors_content(colors: &HashMap<String, u8>) -> String {
    let colors = prepare_colors(colors);
    let mut out = String::new();
    for (name, color) in colors {
        writeln!(out, "'{name}' = {color}");
    }
    out
}

fn colors_buffer(colors: &HashMap<String, u8>) -> Buffer {
    const HEADER_NAME: &str = "[Name]";
    const HEADER_VALUE: &str = "[Value]";

    // Calculate maximum width of color names to align output.
    let colors = prepare_colors(colors);
    let name_width = colors
        .keys()
        .map(|name| name.len())
        .max()
        .unwrap_or(HEADER_NAME.len());

    // Emit formatted colors.
    let mut buf = Buffer::new();
    writeln!(buf, "{HEADER_NAME:<name_width$}   {HEADER_VALUE}");
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
        .collect()
}

/// Returns a TOML-formatted list of theme color names and values.
pub fn theme_content(theme: &Theme) -> String {
    type ColorFn = fn(&Theme) -> u8;
    const COLORS: [(&str, ColorFn); 15] = [
        ("text-fg", |t| t.text_fg),
        ("text-bg", |t| t.text_bg),
        ("select-bg", |t| t.select_bg),
        ("spotlight-bg", |t| t.spotlight_bg),
        ("whitespace-fg", |t| t.whitespace_fg),
        ("accent-fg", |t| t.accent_fg),
        ("echo-fg", |t| t.echo_fg),
        ("prompt-fg", |t| t.prompt_fg),
        ("banner-fg", |t| t.banner_fg),
        ("dirty-fg", |t| t.dirty_fg),
        ("active-bg", |t| t.active_bg),
        ("inactive-bg", |t| t.inactive_bg),
        ("margin-fg", |t| t.margin_fg),
        ("margin-bg", |t| t.margin_bg),
        ("line-fg", |t| t.line_fg),
    ];

    let mut out = String::new();
    for (name, t_fn) in COLORS {
        writeln!(out, "'{name}' = {}", t_fn(theme));
    }
    out
}

/// Returns an ephemeral editor, named `@syntaxes`, containing a list of available syntaxes.
pub fn syntaxes_editor(config: ConfigurationRef) -> EditorRef {
    let buffer = syntaxes_buffer(config.registry.syntaxes());
    Editor::readonly(config, Source::as_ephemeral(SYNTAXES_EDITOR_NAME), buffer).into_ref()
}

/// Returns a formatted list of available syntaxes
pub fn syntaxes_content(syntaxes: &HashMap<String, Syntax>) -> String {
    let syntaxes = prepare_syntaxes(syntaxes);
    let mut out = String::new();
    for name in syntaxes {
        writeln!(out, "{name}");
    }
    out
}

fn syntaxes_buffer(syntaxes: &HashMap<String, Syntax>) -> Buffer {
    let syntaxes = prepare_syntaxes(syntaxes);
    let mut buf = Buffer::new();
    writeln!(buf, "[Syntax]");
    for name in syntaxes {
        writeln!(buf, "{name}");
    }
    buf.set_pos(0);
    buf
}

fn prepare_syntaxes(syntaxes: &HashMap<String, Syntax>) -> Vec<String> {
    let mut syntaxes = syntaxes.keys().cloned().collect::<Vec<_>>();
    syntaxes.sort();
    syntaxes
}
