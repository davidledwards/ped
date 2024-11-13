//! # Editing operations
//!
//! A collection of functions intended to be associated with names of editing
//! operations. These functions serve as the glue between [`Key`](crate::key::Key)s and
//! respective actions in the context of the editing experience.
//!
//! Editing operations are designed to be callable only indirectly through [`OpMap`]
//! instances created by [`init_op_map`]. The mapping of names to functions is captured
//! in [`OP_MAPPINGS`].
//!
//! See [`Bindings`](crate::bind::Bindings) for further details on binding keys
//! at runtime.
use crate::buffer::Buffer;
use crate::editor::{Align, Editor, Storage};
use crate::env::Environment;
use crate::error::{Error, Result};
use crate::io;
use crate::size::{Point, Size};
use crate::workspace::Placement;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::time::SystemTime;

/// A function type that implements an editing operation.
pub type OpFn = fn(&mut Environment) -> Result<Option<Action>>;

/// An action returned by an [`OpFn`] that is carried out by a controller orchestrating
/// calls to such functions.
pub enum Action {
    Quit,
    Alert(String),
    Question(String, Box<AnswerFn>),
}

/// A callback function that handles answers to [`Action::Question`] actions.
pub type AnswerFn =
    dyn for<'a> FnMut(&'a mut Environment, Option<&'a str>) -> Result<Option<Action>>;

/// Map of editing operations to editing functions.
pub type OpMap = HashMap<&'static str, OpFn>;

impl Action {
    fn as_quit() -> Option<Action> {
        Some(Action::Quit)
    }

    fn as_alert(text: &str) -> Option<Action> {
        let action = Action::Alert(text.to_string());
        Some(action)
    }

    fn as_alert_error(e: &Error) -> Option<Action> {
        let action = Action::Alert(e.to_string());
        Some(action)
    }

    fn as_question<F>(prompt: &str, answer_fn: F) -> Option<Action>
    where
        F: for<'a> FnMut(&'a mut Environment, Option<&'a str>) -> Result<Option<Action>> + 'static,
    {
        let action = Action::Question(prompt.to_string(), Box::new(answer_fn));
        Some(action)
    }
}

/// Operation: `quit`
fn quit(_: &mut Environment) -> Result<Option<Action>> {
    // todo: ask to save dirty buffers
    Ok(Action::as_quit())
}

/// Operation: `move-left`
fn move_left(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    editor.move_left(1);
    Ok(None)
}

/// Operation: `move-left-select`
fn move_left_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    editor.move_left(1);
    Ok(None)
}

/// Operation: `move-right`
fn move_right(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    editor.move_right(1);
    Ok(None)
}

/// Operation: `move-right-select`
fn move_right_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    editor.move_right(1);
    Ok(None)
}

/// Operation: `move-up`
fn move_up(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    editor.move_up(1, false);
    Ok(None)
}

/// Operation: `move-up-select`
fn move_up_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    editor.move_up(1, false);
    Ok(None)
}

/// Operation: `move-down`
fn move_down(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    editor.move_down(1, false);
    Ok(None)
}

/// Operation: `move-down-select`
fn move_down_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    editor.move_down(1, false);
    Ok(None)
}

/// Operation: `move-up-page`
fn move_up_page(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    let rows = editor.rows();
    editor.move_up(rows, true);
    Ok(None)
}

/// Operation: `move-up-page-select`
fn move_up_page_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    let rows = editor.rows();
    editor.move_up(rows, true);
    Ok(None)
}

/// Operation: `move-down-page`
fn move_down_page(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    let rows = editor.rows();
    editor.move_down(rows, true);
    Ok(None)
}

/// Operation: `move-down-page-select`
fn move_down_page_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    let rows = editor.rows();
    editor.move_down(rows, true);
    Ok(None)
}

/// Operation: `move-start`
fn move_start(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    editor.move_start();
    Ok(None)
}

/// Operation: `move-start-select`
fn move_start_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    editor.move_start();
    Ok(None)
}

/// Operation: `move-end`
fn move_end(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    editor.move_end();
    Ok(None)
}

/// Operation: `move-end-select`
fn move_end_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    editor.move_end();
    Ok(None)
}

/// Operation: `move-top`
fn move_top(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    editor.move_top();
    Ok(None)
}

/// Operation: `move-top-select`
fn move_top_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    editor.move_top();
    Ok(None)
}

/// Operation: `move-bottom`
fn move_bottom(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_soft_mark();
    editor.move_bottom();
    Ok(None)
}

/// Operation: `move-bottom-select`
fn move_bottom_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.set_soft_mark();
    editor.move_bottom();
    Ok(None)
}

/// Operation: `scroll-up`
fn scroll_up(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().scroll_up(1);
    Ok(None)
}

/// Operation: `scroll-down`
fn scroll_down(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().scroll_down(1);
    Ok(None)
}

/// Operation: `scroll-center`
fn scroll_center(env: &mut Environment) -> Result<Option<Action>> {
    // Rotate through alignment based on current cursor position using following
    // cycle: center -> bottom -> top.
    //
    // If position is not precisely on one of these rows, then start at center. This
    // behavior allows user to quickly align cursor with successive key presses.
    let mut editor = env.active_editor();
    let Size { rows, .. } = editor.size();
    let Point { row, .. } = editor.cursor();
    let align = if row == 0 {
        Align::Center
    } else if row == rows / 2 {
        Align::Bottom
    } else if row == rows - 1 {
        Align::Top
    } else {
        Align::Center
    };
    editor.align_cursor(align);
    Ok(None)
}

/// Operation: `set-mark`
fn set_mark(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    if let Some(_) = editor.set_hard_mark() {
        editor.render();
    }
    Ok(None)
}

/// Operation: `goto-line`
fn goto_line(_: &mut Environment) -> Result<Option<Action>> {
    let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
        let action = if let Some(s) = answer {
            if let Ok(line) = s.parse::<u32>() {
                let line = if line > 0 { line - 1 } else { line };
                env.active_editor().move_line(line, Align::Center);
                None
            } else {
                Action::as_alert(&format!("{s}: invalid line number"))
            }
        } else {
            None
        };
        Ok(action)
    };
    Ok(Action::as_question("goto line:", answer_fn))
}

/// Operation: `insert-line`
fn insert_line(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_mark();
    editor.insert_char('\n');
    Ok(None)
}

/// Operation: `remove-left`
fn remove_left(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_mark();
    let _ = editor.remove_left();
    Ok(None)
}

/// Operation: `remove-right`
fn remove_right(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_mark();
    let _ = editor.remove_right();
    Ok(None)
}

/// Operation: `remove-start`
fn remove_start(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_mark();
    let _ = editor.remove_start();
    Ok(None)
}

/// Operation: `remove-end`
fn remove_end(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    editor.clear_mark();
    let _ = editor.remove_end();
    Ok(None)
}

/// Operation: `copy`
fn copy(env: &mut Environment) -> Result<Option<Action>> {
    let maybe_mark = env.active_editor().clear_mark();
    let text = if let Some(mark) = maybe_mark {
        env.active_editor().copy_mark(mark)
    } else {
        env.active_editor().copy_line()
    };
    env.set_clipboard(text);
    env.active_editor().render();
    Ok(None)
}

/// Operation: `paste`
fn paste(env: &mut Environment) -> Result<Option<Action>> {
    let maybe_text = env.get_clipboard();
    if let Some(text) = maybe_text {
        env.active_editor().insert(text);
    }
    Ok(None)
}

/// Operation: `cut`
fn cut(env: &mut Environment) -> Result<Option<Action>> {
    let maybe_mark = env.active_editor().clear_mark();
    let text = if let Some(mark) = maybe_mark {
        env.active_editor().remove_mark(mark)
    } else {
        env.active_editor().remove_line()
    };
    env.set_clipboard(text);
    env.active_editor().render();
    Ok(None)
}

/// Operation: `open-file`
fn open_file(env: &mut Environment) -> Result<Option<Action>> {
    open_file_internal(env, None)
}

/// Operation: `open-file-top`
fn open_file_top(env: &mut Environment) -> Result<Option<Action>> {
    open_file_internal(env, Some(Placement::Top))
}

/// Operation: `open-file-bottom`
fn open_file_bottom(env: &mut Environment) -> Result<Option<Action>> {
    open_file_internal(env, Some(Placement::Bottom))
}

/// Operation: `open-file-above`
fn open_file_above(env: &mut Environment) -> Result<Option<Action>> {
    open_file_internal(env, Some(Placement::Above(env.active_id())))
}

/// Operation: `open-file-below`
fn open_file_below(env: &mut Environment) -> Result<Option<Action>> {
    open_file_internal(env, Some(Placement::Below(env.active_id())))
}

fn open_file_internal(_: &mut Environment, place: Option<Placement>) -> Result<Option<Action>> {
    let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
        let action = if let Some(file) = answer {
            match open_editor(&file) {
                Ok(editor) => {
                    if let Some(place) = place {
                        env.open_view(editor.to_ref(), place);
                    } else {
                        env.set_view(editor.to_ref());
                    }
                    None
                }
                Err(e) => Action::as_alert_error(&e),
            }
        } else {
            None
        };
        Ok(action)
    };
    Ok(Action::as_question("open file:", answer_fn))
}

pub fn open_editor(path: &str) -> Result<Editor> {
    // Try reading file contents into buffer.
    let mut buffer = Buffer::new();
    let time = match io::read_file(path, &mut buffer) {
        Ok(_) => {
            // Contents read successfully, so fetch time of last modification for use
            // in checking before subsequent write operation.
            io::get_time(path.as_ref()).ok()
        }
        Err(Error::IO { device: _, cause }) if cause.kind() == ErrorKind::NotFound => {
            // File was not found, but still treat this error condition as successful,
            // though note that last modification time is absent to indicate new file.
            None
        }
        Err(e) => {
            // Propagate all other errors.
            return Err(e);
        }
    };

    // Create persistent buffer with position set at top.
    buffer.set_pos(0);
    let editor = Editor::new(Storage::as_persistent(path, time), buffer.to_ref());
    Ok(editor)
}

/// Operation: `save-file`
fn save_file(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.active_editor();
    let action = match editor.storage().clone() {
        Storage::Persistent { path, time } => {
            match time {
                Some(time) if io::get_time(&path)? > time => {
                    // An existing file where modification time in storage is
                    // newer than time read when file was opened, so ask user
                    // to make decision before saving buffer.
                    Action::as_question(
                        "file in storage is newer, save anyway (y/n)?",
                        save_override_callback,
                    )
                }
                _ => {
                    // Either a new file or an existing file where modification
                    // time has changed, so just save buffer.
                    save_editor(&mut editor, &path)
                }
            }
        }
        Storage::Transient { name: _ } => {
            // User must provide path in order to save buffer since storage is
            // transient.
            Action::as_question("save as:", save_as_callback)
        }
    };
    Ok(action)
}

/// Operation: `save-file-as`
fn save_file_as(_: &mut Environment) -> Result<Option<Action>> {
    Ok(Action::as_question("save as:", save_as_callback))
}

fn save_as_callback(env: &mut Environment, answer: Option<&str>) -> Result<Option<Action>> {
    let action = if let Some(path) = answer {
        let mut editor = env.active_editor();
        save_editor(&mut editor, path)
    } else {
        None
    };
    Ok(action)
}

fn save_override_callback(env: &mut Environment, answer: Option<&str>) -> Result<Option<Action>> {
    let action = match answer {
        Some(yes_no) if yes_no.to_lowercase() == "y" => {
            let mut editor = env.active_editor();
            let path = editor
                .storage()
                .path()
                .unwrap_or_else(|| panic!("path expected for editor"));
            save_editor(&mut editor, &path)
        }
        _ => None,
    };
    Ok(action)
}

fn save_editor(editor: &mut Editor, path: &str) -> Option<Action> {
    let time = save_buffer(&editor.buffer(), path);
    match time {
        Ok(time) => {
            let storage = Storage::as_persistent(path, Some(time));
            editor.clear_dirty(storage);
            Action::as_alert(&format!("{path}: saved"))
        }
        Err(e) => Action::as_alert_error(&e),
    }
}

fn save_buffer(buffer: &Buffer, path: &str) -> Result<SystemTime> {
    let _ = io::write_file(path, buffer)?;
    let time = io::get_time(path)?;
    Ok(time)
}

/// Operation: `close-window`
fn close_window(env: &mut Environment) -> Result<Option<Action>> {
    let action = env
        .close_view(env.active_id())
        .map(|_| None)
        .unwrap_or_else(|| Action::as_alert("cannot close only window"));
    Ok(action)
}

/// Operation: `top-window`
fn top_window(env: &mut Environment) -> Result<Option<Action>> {
    env.top_view();
    Ok(None)
}

/// Operation: `bottom-window`
fn bottom_window(env: &mut Environment) -> Result<Option<Action>> {
    env.bottom_view();
    Ok(None)
}

/// Operation: `prev-window`
fn prev_window(env: &mut Environment) -> Result<Option<Action>> {
    env.prev_view();
    Ok(None)
}

/// Operation: `next-window`
fn next_window(env: &mut Environment) -> Result<Option<Action>> {
    env.next_view();
    Ok(None)
}

/// Predefined mapping of editing operations to editing functions.
const OP_MAPPINGS: [(&'static str, OpFn); 46] = [
    // --- exit and cancellation ---
    ("quit", quit),
    // --- navigation and selection ---
    ("move-left", move_left),
    ("move-left-select", move_left_select),
    ("move-right", move_right),
    ("move-right-select", move_right_select),
    ("move-up", move_up),
    ("move-up-select", move_up_select),
    ("move-down", move_down),
    ("move-down-select", move_down_select),
    ("move-up-page", move_up_page),
    ("move-up-page-select", move_up_page_select),
    ("move-down-page", move_down_page),
    ("move-down-page-select", move_down_page_select),
    ("move-start", move_start),
    ("move-start-select", move_start_select),
    ("move-end", move_end),
    ("move-end-select", move_end_select),
    ("move-top", move_top),
    ("move-top-select", move_top_select),
    ("move-bottom", move_bottom),
    ("move-bottom-select", move_bottom_select),
    ("scroll-up", scroll_up),
    ("scroll-down", scroll_down),
    ("scroll-center", scroll_center),
    ("set-mark", set_mark),
    ("goto-line", goto_line),
    // --- insertion and removal ---
    ("insert-line", insert_line),
    ("remove-left", remove_left),
    ("remove-right", remove_right),
    ("remove-start", remove_start),
    ("remove-end", remove_end),
    // --- selection actions ---
    ("copy", copy),
    ("paste", paste),
    ("cut", cut),
    // --- file handling ---
    ("open-file", open_file),
    ("open-file-top", open_file_top),
    ("open-file-bottom", open_file_bottom),
    ("open-file-above", open_file_above),
    ("open-file-below", open_file_below),
    ("save-file", save_file),
    ("save-file-as", save_file_as),
    // --- window handling ---
    ("close-window", close_window),
    ("top-window", top_window),
    ("bottom-window", bottom_window),
    ("prev-window", prev_window),
    ("next-window", next_window),
];

pub fn init_op_map() -> OpMap {
    let mut op_map = OpMap::new();
    for (op, op_fn) in OP_MAPPINGS {
        op_map.insert(op, op_fn);
    }
    op_map
}
