//! Editing operations.
//!
//! A collection of functions intended to be associated with canonical names of
//! editing operations. These functions serve as the glue between a [`Key`] and
//! its respective action in the context of the editing experience.
//!
//! See [`BindingMap`](crate::bind::BindingMap) for further details on binding keys
//! at runtime.
use crate::control::Controller;
use crate::editor::Align;
use crate::error::Result;
use crate::key::Key;

/// A function type that implements an editing operation.
pub type OpFn = fn(&mut Controller, &Key) -> Result<Action>;

/// A function type that implements a continuation of an editing operation.
pub type ContinueFn = Box<dyn FnMut(&mut Controller, &Key) -> Result<Action>>;

/// An action returned by an editing operation that is to be carried out by the
/// [`Controller`].
pub enum Action {
    Nothing,
    Quit,
    Continue(ContinueFn),
}

pub fn special(con: &mut Controller, key: &Key) -> Result<Action> {
    con.workspace().alert("ctrl-c");
    Ok(Action::Continue(Box::new(special_cont)))
}

fn special_cont(con: &mut Controller, key: &Key) -> Result<Action> {
    con.workspace().alert(format!("ctrl-c {key:?}").as_str());
    Ok(Action::Nothing)
}

/// insert-char
pub fn insert_char(con: &mut Controller, key: &Key) -> Result<Action> {
    match key {
        Key::Char(c) => {
            con.editor().insert_char(*c);
            Ok(Action::Nothing)
        }
        _ => panic!("{key:?}: expecting Key::Char"),
    }
}

/// insert-line
pub fn insert_line(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().insert_char('\n');
    Ok(Action::Nothing)
}

/// delete-char-left
pub fn delete_char_left(con: &mut Controller, _: &Key) -> Result<Action> {
    // todo: should we return deleted char in result?
    let _ = con.editor().delete_left();
    Ok(Action::Nothing)
}

/// delete-char-right
pub fn delete_char_right(con: &mut Controller, _: &Key) -> Result<Action> {
    // todo: should we return deleted char in result?
    let _ = con.editor().delete_right();
    Ok(Action::Nothing)
}

/// move-up
pub fn move_up(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_up();
    Ok(Action::Nothing)
}

/// move-down
pub fn move_down(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_down();
    Ok(Action::Nothing)
}

/// move-left
pub fn move_left(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_left();
    Ok(Action::Nothing)
}

/// move-right
pub fn move_right(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_right();
    Ok(Action::Nothing)
}

/// move-page-up
pub fn move_page_up(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_page_up();
    Ok(Action::Nothing)
}

/// move-page-down
pub fn move_page_down(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_page_down();
    Ok(Action::Nothing)
}

/// move-top
pub fn move_top(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_top();
    Ok(Action::Nothing)
}

/// move-bottom
pub fn move_bottom(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_bottom();
    Ok(Action::Nothing)
}

/// scroll-up
pub fn scroll_up(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().scroll_up();
    Ok(Action::Nothing)
}

/// scroll-down
pub fn scroll_down(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().scroll_down();
    Ok(Action::Nothing)
}

/// move-begin-line
pub fn move_begin_line(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_beg();
    Ok(Action::Nothing)
}

/// move-end-line
pub fn move_end_line(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().move_end();
    Ok(Action::Nothing)
}

/// redraw
pub fn redraw(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().draw();
    Ok(Action::Nothing)
}

/// redraw-and-center
pub fn redraw_and_center(con: &mut Controller, _: &Key) -> Result<Action> {
    con.editor().align_cursor(Align::Center);
    con.editor().draw();
    Ok(Action::Nothing)
}

/// quit
pub fn quit(_: &mut Controller, _: &Key) -> Result<Action> {
    // FIXME: ask to save dirty buffers
    Ok(Action::Quit)
}
