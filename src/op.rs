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
use crate::error::Result;
use crate::session::Session;
use crate::workspace::Placement;

use std::collections::HashMap;

/// A function type that implements an editing operation.
pub type OpFn = fn(&mut Session) -> Result<Option<Action>>;

/// Map of editing operations to editing functions.
pub type OpMap = HashMap<&'static str, OpFn>;

pub enum Action {
    Quit,
    Alert(String),
}

/// Operation: `insert-line`
fn insert_line(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().insert_char('\n');
    Ok(None)
}

/// Operation: `delete-char-left`
fn delete_char_left(session: &mut Session) -> Result<Option<Action>> {
    // todo: should we return deleted char in result?
    let _ = session.active_editor().delete_left();
    Ok(None)
}

/// Operation: `delete-char-right`
fn delete_char_right(session: &mut Session) -> Result<Option<Action>> {
    let _ = session.active_editor().delete_right();
    Ok(None)
}

/// Operation: `move-up`
fn move_up(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_up();
    Ok(None)
}

/// Operation: `move-down`
fn move_down(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_down();
    Ok(None)
}

/// Operation: `move-left`
fn move_left(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_left();
    Ok(None)
}

/// Operation: `move-right`
fn move_right(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_right();
    Ok(None)
}

/// Operation: `move-page-up`
fn move_page_up(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_page_up();
    Ok(None)
}

/// Operation: `move-page-down`
fn move_page_down(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_page_down();
    Ok(None)
}

/// Operation: `move-top`
fn move_top(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_top();
    Ok(None)
}

/// Operation: `move-bottom`
fn move_bottom(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_bottom();
    Ok(None)
}

/// Operation: `scroll-up`
fn scroll_up(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().scroll_up();
    Ok(None)
}

/// Operation: `scroll-down`
fn scroll_down(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().scroll_down();
    Ok(None)
}

/// Operation: `move-begin-line`
fn move_begin_line(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_beg();
    Ok(None)
}

/// Operation: `move-end-line`
fn move_end_line(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().move_end();
    Ok(None)
}

/// Operation: `redraw`
fn redraw(session: &mut Session) -> Result<Option<Action>> {
    session.active_editor().draw();
    Ok(None)
}

/// Operation: `redraw-and-center`
fn redraw_and_center(session: &mut Session) -> Result<Option<Action>> {
    let mut editor = session.active_editor();
    editor.align_cursor(Align::Center);
    editor.draw();
    Ok(None)
}

/// Operation: `quit`
fn quit(_: &mut Session) -> Result<Option<Action>> {
    // FIXME: ask to save dirty buffers
    Ok(Some(Action::Quit))
}

fn open_window_top(session: &mut Session) -> Result<Option<Action>> {
    // FIXME: move alerting from session to here
    let _ = session.add_view(Placement::Top);
    Ok(None)
}

fn open_window_bottom(session: &mut Session) -> Result<Option<Action>> {
    let _ = session.add_view(Placement::Bottom);
    Ok(None)
}

fn open_window_above(session: &mut Session) -> Result<Option<Action>> {
    let _ = session.add_view(Placement::Above(session.active_id()));
    Ok(None)
}

fn open_window_below(session: &mut Session) -> Result<Option<Action>> {
    let _ = session.add_view(Placement::Below(session.active_id()));
    Ok(None)
}

fn close_window(session: &mut Session) -> Result<Option<Action>> {
    let _ = session.remove_view(session.active_id());
    Ok(None)
}

fn prev_window(session: &mut Session) -> Result<Option<Action>> {
    session.prev_view();
    Ok(None)
}

fn next_window(session: &mut Session) -> Result<Option<Action>> {
    session.next_view();
    Ok(None)
}

/// Predefined mapping of editing operations to editing functions.
const OP_MAPPINGS: [(&'static str, OpFn); 25] = [
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
    ("redraw-and-center", redraw_and_center),
    ("quit", quit),
    ("open-window-top", open_window_top),
    ("open-window-bottom", open_window_bottom),
    ("open-window-above", open_window_above),
    ("open-window-below", open_window_below),
    ("close-window", close_window),
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
