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
use crate::editor::Align;
use crate::env::Environment;
use crate::error::Result;
use crate::size::{Point, Size};
use crate::workspace::Placement;

use std::collections::HashMap;

/// A function type that implements an editing operation.
pub type OpFn = fn(&mut Environment) -> Result<Option<Action>>;

/// Map of editing operations to editing functions.
pub type OpMap = HashMap<&'static str, OpFn>;

pub type AnswerFn = dyn FnMut(&mut Environment, Option<&str>) -> Result<Option<Action>>;

pub enum Action {
    Quit,
    Alert(String),
    Question(String, Box<AnswerFn>),
}

fn open_file(env: &mut Environment) -> Result<Option<Action>> {
    let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
        // FIXME: for testing purposes
        if let Some(file) = answer {
            if let Some(id) = env.open_view(Placement::Bottom) {
                if let Some(editor) = env.get_editor(id) {
                    editor.borrow_mut().insert_chars(&file.chars().collect());
                }
                Ok(None)
            } else {
                Ok(Some(Action::Alert(format!(
                    "{file}: not enough room for new window"
                ))))
            }
        } else {
            Ok(None)
        }
    };
    Ok(Some(Action::Question(
        "open file:".to_string(),
        Box::new(answer_fn),
    )))
}

/// Operation: `insert-line`
fn insert_line(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().insert_char('\n');
    Ok(None)
}

/// Operation: `delete-char-left`
fn delete_char_left(env: &mut Environment) -> Result<Option<Action>> {
    // todo: should we return deleted char in result?
    let _ = env.active_editor().delete_left();
    Ok(None)
}

/// Operation: `delete-char-right`
fn delete_char_right(env: &mut Environment) -> Result<Option<Action>> {
    let _ = env.active_editor().delete_right();
    Ok(None)
}

/// Operation: `move-up`
fn move_up(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_up();
    Ok(None)
}

/// Operation: `move-down`
fn move_down(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_down();
    Ok(None)
}

/// Operation: `move-left`
fn move_left(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_left();
    Ok(None)
}

/// Operation: `move-right`
fn move_right(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_right();
    Ok(None)
}

/// Operation: `move-page-up`
fn move_page_up(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_page_up();
    Ok(None)
}

/// Operation: `move-page-down`
fn move_page_down(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_page_down();
    Ok(None)
}

/// Operation: `move-top`
fn move_top(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_top();
    Ok(None)
}

/// Operation: `move-bottom`
fn move_bottom(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_bottom();
    Ok(None)
}

/// Operation: `scroll-up`
fn scroll_up(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().scroll_up();
    Ok(None)
}

/// Operation: `scroll-down`
fn scroll_down(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().scroll_down();
    Ok(None)
}

/// Operation: `move-begin-line`
fn move_begin_line(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_beg();
    Ok(None)
}

/// Operation: `move-end-line`
fn move_end_line(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().move_end();
    Ok(None)
}

/// Operation: `redraw`
fn redraw(env: &mut Environment) -> Result<Option<Action>> {
    env.active_editor().draw();
    Ok(None)
}

/// Operation: `scroll-center`
fn scroll_center(env: &mut Environment) -> Result<Option<Action>> {
    // Rotate through alignment based on current cursor position using following
    // cycle: center -> bottom -> top. If position is not precisely on one of
    // these rows, then start at center. This behavior allows user to quickly
    // align cursor with successive key presses.
    let mut editor = env.active_editor();
    let Size { rows, .. } = editor.get_size();
    let Point { row, .. } = editor.get_cursor();
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
    editor.draw();
    Ok(None)
}

/// Operation: `quit`
fn quit(_: &mut Environment) -> Result<Option<Action>> {
    // FIXME: ask to save dirty buffers
    Ok(Some(Action::Quit))
}

fn open_window_top(env: &mut Environment) -> Result<Option<Action>> {
    let action = env
        .open_view(Placement::Top)
        .map(|_| None)
        .unwrap_or_else(|| Some(Action::Alert("out of window space".to_string())));
    Ok(action)
}

fn open_window_bottom(env: &mut Environment) -> Result<Option<Action>> {
    let action = env
        .open_view(Placement::Bottom)
        .map(|_| None)
        .unwrap_or_else(|| Some(Action::Alert("out of window space".to_string())));
    Ok(action)
}

fn open_window_above(env: &mut Environment) -> Result<Option<Action>> {
    let action = env
        .open_view(Placement::Above(env.active_id()))
        .map(|_| None)
        .unwrap_or_else(|| Some(Action::Alert("out of window space".to_string())));
    Ok(action)
}

fn open_window_below(env: &mut Environment) -> Result<Option<Action>> {
    let action = env
        .open_view(Placement::Below(env.active_id()))
        .map(|_| None)
        .unwrap_or_else(|| Some(Action::Alert("out of window space".to_string())));
    Ok(action)
}

fn close_window(env: &mut Environment) -> Result<Option<Action>> {
    let action = env
        .close_view(env.active_id())
        .map(|_| None)
        .unwrap_or_else(|| Some(Action::Alert("cannot close only window".to_string())));
    Ok(action)
}

fn prev_window(env: &mut Environment) -> Result<Option<Action>> {
    env.prev_view();
    Ok(None)
}

fn next_window(env: &mut Environment) -> Result<Option<Action>> {
    env.next_view();
    Ok(None)
}

/// Predefined mapping of editing operations to editing functions.
const OP_MAPPINGS: [(&'static str, OpFn); 26] = [
    ("insert-line", insert_line),
    ("delete-char-left", delete_char_left),
    ("delete-char-right", delete_char_right),
    ("move-up", move_up),
    ("move-down", move_down),
    ("move-left", move_left),
    ("move-right", move_right),
    ("move-page-up", move_page_up),
    ("move-page-down", move_page_down),
    ("move-top", move_top),
    ("move-bottom", move_bottom),
    ("scroll-up", scroll_up),
    ("scroll-down", scroll_down),
    ("move-begin-line", move_begin_line),
    ("move-end-line", move_end_line),
    ("redraw", redraw),
    ("scroll-center", scroll_center),
    ("quit", quit),
    ("open-window-top", open_window_top),
    ("open-window-bottom", open_window_bottom),
    ("open-window-above", open_window_above),
    ("open-window-below", open_window_below),
    ("close-window", close_window),
    ("prev-window", prev_window),
    ("next-window", next_window),
    // FIXME: added for testing
    ("open-file", open_file),
];

pub fn init_op_map() -> OpMap {
    let mut op_map = OpMap::new();
    for (op, op_fn) in OP_MAPPINGS {
        op_map.insert(op, op_fn);
    }
    op_map
}
