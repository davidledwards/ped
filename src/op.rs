//! Editing operations.
//!
//! A collection of functions intended to be associated with canonical names of
//! editing operations. These functions serve as the glue between a [`Key`] and
//! its respective action in the context of the editing experience.
//!
//! See [`BindingMap`](crate::bind::BindingMap) for further details on binding keys
//! at runtime.

use crate::editor::{Editor, Focus};
use crate::error::Result;
use crate::key::Key;

/// insert-char
pub fn insert_char(editor: &mut Editor, key: &Key) -> Result<()> {
    match key {
        Key::Char(c) => {
            editor.insert_char(*c);
            Ok(())
        }
        _ => Err(format!("{key:?}: expecting Key::Char").into()),
    }
}

/// insert-line
pub fn insert_line(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.insert_char('\n');
    Ok(())
}

/// delete-char-left
pub fn delete_char_left(editor: &mut Editor, _: &Key) -> Result<()> {
    // todo: should we return deleted char in result?
    let _ = editor.delete_left();
    Ok(())
}

/// delete-char-right
pub fn delete_char_right(editor: &mut Editor, _: &Key) -> Result<()> {
    // todo: should we return deleted char in result?
    let _ = editor.delete_right();
    Ok(())
}

/// move-up
pub fn move_up(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_up();
    Ok(())
}

/// move-down
pub fn move_down(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_down();
    Ok(())
}

/// move-left
pub fn move_left(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_left();
    Ok(())
}

/// move-right
pub fn move_right(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_right();
    Ok(())
}

/// move-page-up
pub fn move_page_up(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_page_up();
    Ok(())
}

/// move-page-down
pub fn move_page_down(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_page_down();
    Ok(())
}

/// move-top
pub fn move_top(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_top();
    Ok(())
}

/// move-bottom
pub fn move_bottom(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_bottom();
    Ok(())
}

/// scroll-up
pub fn scroll_up(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.scroll_up();
    Ok(())
}

/// scroll-down
pub fn scroll_down(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.scroll_down();
    Ok(())
}

/// move-begin-line
pub fn move_begin_line(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_beg();
    Ok(())
}

/// move-end-line
pub fn move_end_line(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_end();
    Ok(())
}

/// redraw
pub fn redraw(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.redraw();
    Ok(())
}

/// redraw-and-center
pub fn redraw_and_center(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.redraw_focus(Focus::Auto);
    Ok(())
}
