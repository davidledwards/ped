//! Useful functions that operate on editors.

use crate::buffer::Buffer;
use crate::config::ConfigurationRef;
use crate::editor::{EditorBuilder, EditorRef};
use crate::env::Environment;
use crate::error::{Error, Result};
use crate::io;
use crate::source::Source;
use crate::sys::{self, AsString};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::SystemTime;

/// Returns the path associated with `editor`.
pub fn path_of(editor: &EditorRef) -> PathBuf {
    if let Source::File(path, _) = editor.borrow().source() {
        PathBuf::from(path)
    } else {
        PathBuf::from("")
    }
}

/// Returns the source associated with `editor`.
pub fn source_of(editor: &EditorRef) -> String {
    editor.borrow().source().to_string()
}

/// Returns the base directory of the path associated with `editor` so long as the
/// editor source is a _file_, otherwise the current working directory is assumed.
///
/// `None` is returned if the base directory cannot be determined, possibly from a
/// failure to get the current working directory.
pub fn base_dir(editor: &EditorRef) -> PathBuf {
    if let Source::File(path, _) = editor.borrow().source() {
        sys::base_dir(path)
    } else {
        sys::working_dir()
    }
}

/// Returns the base directory of the active editor.
pub fn derive_dir(env: &mut Environment) -> PathBuf {
    derive_dir_from(env.get_active_editor())
}

/// Returns the base directory derived from `editor`, which is canonicalized so long
/// as no failures occur along the way, otherwise it resorts to a directory path of
/// `"."`
pub fn derive_dir_from(editor: &EditorRef) -> PathBuf {
    base_dir(editor)
        .canonicalize()
        .unwrap_or_else(|_| sys::this_dir())
}

/// Returns `true` if the source of `editor` is a _file_.
pub fn is_file(editor: &EditorRef) -> bool {
    editor.borrow().source().is_file()
}

/// Returns `true` if the source of `editor` is an _ephemeral_.
pub fn is_ephemeral(editor: &EditorRef) -> bool {
    editor.borrow().source().is_ephemeral()
}

/// Returns `true` if the source of `editor` is a _file_ and is dirty.
pub fn is_dirty_file(editor: &EditorRef) -> bool {
    let editor = editor.borrow();
    editor.source().is_file() && editor.is_dirty()
}

/// Returns `true` if `editor` has a modification time older than the modification time
/// of the file in storage.
pub fn stale_editor(editor: &EditorRef) -> Result<bool> {
    let editor = editor.borrow();
    let stale = if let Source::File(path, Some(timestamp)) = editor.source() {
        io::get_time(path)? > *timestamp
    } else {
        false
    };
    Ok(stale)
}

/// Returns an ordered collection of _dirty_ editors.
pub fn dirty_editors(env: &Environment) -> Vec<EditorRef> {
    env.editor_map()
        .iter()
        .filter(|(_, e)| is_dirty_file(e))
        .map(|(_, e)| e.clone())
        .collect()
}

/// Reads the file at `path` and returns a new editor.
pub fn open_editor(config: ConfigurationRef, path: &str) -> Result<EditorRef> {
    // Try reading file contents into buffer.
    let mut buffer = Buffer::new();
    let time = match io::read_file(path, &mut buffer) {
        Ok(_) => {
            // Contents read successfully, so fetch time of last modification for use
            // in checking before subsequent write operation.
            io::get_time(path).ok()
        }
        Err(Error::Io { path: _, cause }) if cause.kind() == ErrorKind::NotFound => {
            // File was not found, but still treat this error condition as successful,
            // though note that last modification time is absent to indicate new file.
            None
        }
        Err(e) => {
            // Propagate all other errors.
            return Err(e);
        }
    };

    // Create file buffer with position set at top.
    buffer.set_pos(0);
    let editor = EditorBuilder::new(config)
        .source(Source::as_file(path, time))
        .buffer(buffer)
        .build();
    Ok(editor.into_ref())
}

/// Combines [`write_editor`] and [`update_editor`] into a single operation.
pub fn save_editor(editor: &EditorRef) -> Result<()> {
    save_editor_as(editor, None)
}

/// Combines [`write_editor`] and [`update_editor`] into a single operation, saving
/// the editor using the optional `path`, otherwise it derives the path from `editor`.
pub fn save_editor_as(editor: &EditorRef, path: Option<&str>) -> Result<()> {
    let path = path
        .map(|path| path.to_string())
        .unwrap_or_else(|| path_of(editor).as_string());
    write_editor(editor, &path).map(|time| update_editor(editor, &path, time))
}

/// Writes the buffer of `editor` to `path` and returns the resulting file modification
/// time.
pub fn write_editor(editor: &EditorRef, path: &str) -> Result<SystemTime> {
    let _ = io::write_file(path, &editor.borrow().buffer(), editor.borrow().get_crlf())?;
    io::get_time(path)
}

/// Clears the dirty flag on `editor` and sets its source as _file_ using `path` and
/// its modification `timestamp`.
pub fn update_editor(editor: &EditorRef, path: &str, timestamp: SystemTime) {
    let mut editor = editor.borrow_mut();
    editor.assume(Source::as_file(path, Some(timestamp)));
    editor.clear_dirty();
}

/// Returns an ordered collection of editor ids and editors for those editors that are
/// not attached to a window.
///
/// When `ephemerals` is `true`, the collection also contains ephemeral editors,
/// otherwise they are filtered out.
pub fn unattached_editors(env: &Environment, ephemerals: bool) -> Vec<(u32, EditorRef)> {
    let attached = env.view_map().values().cloned().collect::<Vec<_>>();
    env.editor_map()
        .iter()
        .filter(|(id, e)| !attached.contains(id) && (is_file(e) || is_ephemeral(e) == ephemerals))
        .map(|(id, e)| (*id, e.clone()))
        .collect()
}

/// Returns the editor id and editor of the previous unattached editor relative to the
/// editor in the current window, or `None` if all editors are attached.
pub fn prev_unattached_editor(env: &Environment) -> Option<(u32, EditorRef)> {
    let editors = unattached_editors(env, false);
    if editors.len() > 0 {
        let editor_id = env.get_active_editor_id();
        let index = editors
            .iter()
            .rev()
            .position(|(id, _)| *id < editor_id)
            .unwrap_or(0);
        Some(editors[editors.len() - index - 1].clone())
    } else {
        None
    }
}

/// Returns the editor id and editor of the next unattached editor relative to the
/// editor in the current window, or `None` if all editors are attached.
pub fn next_unattached_editor(env: &Environment) -> Option<(u32, EditorRef)> {
    let editors = unattached_editors(env, false);
    if editors.len() > 0 {
        let editor_id = env.get_active_editor_id();
        let index = editors
            .iter()
            .position(|(id, _)| *id > editor_id)
            .unwrap_or(0);
        Some(editors[index].clone())
    } else {
        None
    }
}
