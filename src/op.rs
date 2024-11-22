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
use crate::complete::{self, CompleterAction, CompleterEvent, CompleterFn};
use crate::editor::{self, Align, EditorRef, Storage};
use crate::env::{Environment, Focus};
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
    Echo(String),
    Question(String, Box<AnswerFn>, Option<Box<CompleterFn>>),
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

    fn as_echo(text: &str) -> Option<Action> {
        let action = Action::Echo(text.to_string());
        Some(action)
    }

    fn as_echo_error(e: &Error) -> Option<Action> {
        let action = Action::Echo(e.to_string());
        Some(action)
    }

    fn as_question<A, B>(prompt: &str, answer_fn: A, complete_fn: B) -> Option<Action>
    where
        A: for<'a> FnMut(&'a mut Environment, Option<&'a str>) -> Result<Option<Action>> + 'static,
        B: Fn(CompleterEvent) -> Option<CompleterAction> + 'static,
    {
        let action = Action::Question(
            prompt.to_string(),
            Box::new(answer_fn),
            Some(Box::new(complete_fn)),
        );
        Some(action)
    }
}

/// Operation: `quit`
fn quit(env: &mut Environment) -> Result<Option<Action>> {
    let action = quit_continue(env, dirty_editors(env));
    Ok(action)
}

/// Continues the process of saving editors before quitting until `dirty` is empty,
/// an error occurs during saving, or the process is cancelled.
fn quit_continue(env: &mut Environment, dirty: Vec<u32>) -> Option<Action> {
    if let Some(editor_id) = dirty.first().cloned() {
        let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
            quit_save_answer(env, answer, dirty.clone())
        };
        let path = path_of(&get_editor(env, editor_id));
        Action::as_question(&prompt_save(&path), answer_fn, complete::yes_no_all)
    } else {
        Action::as_quit()
    }
}

/// Question callback when saving dirty editors during the quit process.
fn quit_save_answer(
    env: &mut Environment,
    answer: Option<&str>,
    dirty: Vec<u32>,
) -> Result<Option<Action>> {
    let action = match answer {
        Some(yes_no) if yes_no == "y" => quit_save_first(env, dirty),
        Some(yes_no) if yes_no == "a" => quit_save_all(env, dirty),
        Some(yes_no) if yes_no == "n" => {
            let dirty = dirty.iter().cloned().skip(1).collect();
            quit_continue(env, dirty)
        }
        Some(_) => {
            let path = path_of(&get_editor(env, dirty[0]));
            let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
                quit_save_answer(env, answer, dirty.clone())
            };
            Action::as_question(&prompt_save(&path), answer_fn, complete::yes_no_all)
        }
        None => None,
    };
    Ok(action)
}

/// Saves the first editor in `dirty` and then continues to the next editor.
fn quit_save_first(env: &mut Environment, dirty: Vec<u32>) -> Option<Action> {
    let editor_id = *dirty.get(0).expect("expecting at least one dirty editor");
    let editor = get_editor(env, editor_id);
    match stale_editor(editor) {
        Ok(true) => {
            let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
                quit_save_override_answer(env, answer, dirty.clone())
            };
            let path = path_of(editor);
            Action::as_question(&prompt_save_anyway(&path), answer_fn, complete::yes_no)
        }
        Ok(false) => {
            if let Err(e) = save_editor(editor) {
                Action::as_echo_error(&e)
            } else {
                let dirty = dirty.iter().cloned().skip(1).collect();
                quit_continue(env, dirty)
            }
        }
        Err(e) => Action::as_echo_error(&e),
    }
}

/// Saves all editors in `dirty`.
fn quit_save_all(env: &mut Environment, dirty: Vec<u32>) -> Option<Action> {
    let mut dirty_iter = dirty.iter().cloned();
    while let Some(editor_id) = dirty_iter.next() {
        let editor = get_editor(env, editor_id);
        match stale_editor(editor) {
            Ok(true) => {
                let mut dirty = vec![editor_id];
                dirty.extend(dirty_iter);
                let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
                    quit_save_override_answer(env, answer, dirty.clone())
                };
                let path = path_of(editor);
                return Action::as_question(
                    &prompt_save_anyway(&path),
                    answer_fn,
                    complete::yes_no,
                );
            }
            Ok(false) => {
                if let Err(e) = save_editor(editor) {
                    return Action::as_echo_error(&e);
                }
            }
            Err(e) => {
                return Action::as_echo_error(&e);
            }
        }
    }
    Action::as_quit()
}

/// Question callback when requesting override to save an editor during the quit
/// process.
fn quit_save_override_answer(
    env: &mut Environment,
    answer: Option<&str>,
    dirty: Vec<u32>,
) -> Result<Option<Action>> {
    let action = match answer {
        Some(yes_no) if yes_no == "y" => {
            let editor = get_editor(env, dirty[0]);
            if let Err(e) = save_editor(editor) {
                Action::as_echo_error(&e)
            } else {
                let dirty = dirty.iter().cloned().skip(1).collect();
                quit_continue(env, dirty)
            }
        }
        Some(yes_no) if yes_no == "n" => {
            let dirty = dirty.iter().cloned().skip(1).collect();
            quit_continue(env, dirty)
        }
        Some(_) => {
            let path = path_of(&get_editor(env, dirty[0]));
            let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
                quit_save_override_answer(env, answer, dirty.clone())
            };
            Action::as_question(&prompt_save_anyway(&path), answer_fn, complete::yes_no)
        }
        None => None,
    };
    Ok(action)
}

/// Operation: `help`
fn help(_: &mut Environment) -> Result<Option<Action>> {
    // open @help editor at bottom
    // write help text to editpr
    Ok(Action::as_echo("help not yet implemented"))
}

/// Operation: `move-left`
fn move_left(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_left(1);
    Ok(None)
}

/// Operation: `move-left-select`
fn move_left_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_left(1);
    Ok(None)
}

/// Operation: `move-right`
fn move_right(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_right(1);
    Ok(None)
}

/// Operation: `move-right-select`
fn move_right_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_right(1);
    Ok(None)
}

/// Operation: `move-up`
fn move_up(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_up(1, false);
    Ok(None)
}

/// Operation: `move-up-select`
fn move_up_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_up(1, false);
    Ok(None)
}

/// Operation: `move-down`
fn move_down(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_down(1, false);
    Ok(None)
}

/// Operation: `move-down-select`
fn move_down_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_down(1, false);
    Ok(None)
}

/// Operation: `move-up-page`
fn move_up_page(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    let rows = editor.rows();
    editor.move_up(rows, true);
    Ok(None)
}

/// Operation: `move-up-page-select`
fn move_up_page_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    let rows = editor.rows();
    editor.move_up(rows, true);
    Ok(None)
}

/// Operation: `move-down-page`
fn move_down_page(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    let rows = editor.rows();
    editor.move_down(rows, true);
    Ok(None)
}

/// Operation: `move-down-page-select`
fn move_down_page_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    let rows = editor.rows();
    editor.move_down(rows, true);
    Ok(None)
}

/// Operation: `move-start`
fn move_start(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_start();
    Ok(None)
}

/// Operation: `move-start-select`
fn move_start_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_start();
    Ok(None)
}

/// Operation: `move-end`
fn move_end(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_end();
    Ok(None)
}

/// Operation: `move-end-select`
fn move_end_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_end();
    Ok(None)
}

/// Operation: `move-top`
fn move_top(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_top();
    Ok(None)
}

/// Operation: `move-top-select`
fn move_top_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_top();
    Ok(None)
}

/// Operation: `move-bottom`
fn move_bottom(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_bottom();
    Ok(None)
}

/// Operation: `move-bottom-select`
fn move_bottom_select(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_bottom();
    Ok(None)
}

/// Operation: `scroll-up`
fn scroll_up(env: &mut Environment) -> Result<Option<Action>> {
    env.get_editor().borrow_mut().scroll_up(1);
    Ok(None)
}

/// Operation: `scroll-down`
fn scroll_down(env: &mut Environment) -> Result<Option<Action>> {
    env.get_editor().borrow_mut().scroll_down(1);
    Ok(None)
}

/// Operation: `scroll-center`
fn scroll_center(env: &mut Environment) -> Result<Option<Action>> {
    // Rotate through alignment based on current cursor position using following
    // cycle: center -> bottom -> top.
    //
    // If position is not precisely on one of these rows, then start at center. This
    // behavior allows user to quickly align cursor with successive key presses.
    let mut editor = env.get_editor().borrow_mut();
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
    let mut editor = env.get_editor().borrow_mut();
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
                env.get_editor().borrow_mut().move_line(line, Align::Center);
                None
            } else {
                Action::as_echo(&echo_invalid_line(s))
            }
        } else {
            None
        };
        Ok(action)
    };
    Ok(Action::as_question(
        PROMPT_GOTO_LINE,
        answer_fn,
        complete::number::<u32>,
    ))
}

/// Operation: `insert-line`
fn insert_line(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_mark();
    editor.insert_char('\n');
    Ok(None)
}

/// Operation: `insert-tab`
fn insert_tab(env: &mut Environment) -> Result<Option<Action>> {
    let tab_size = env.workspace().config().settings.tab_size;
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_mark();
    let n = tab_size - (editor.location().col as usize % tab_size);
    editor.insert_str(&" ".repeat(n));
    Ok(None)
}

/// Operation: `remove-left`
fn remove_left(env: &mut Environment) -> Result<Option<Action>> {
    let maybe_mark = env.get_editor().borrow_mut().clear_mark();
    if let Some(mark) = maybe_mark {
        let text = env.get_editor().borrow_mut().remove_mark(mark);
        env.set_clipboard(text);
    } else {
        let _ = env.get_editor().borrow_mut().remove_left();
    }
    Ok(None)
}

/// Operation: `remove-right`
fn remove_right(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_mark();
    let _ = editor.remove_right();
    Ok(None)
}

/// Operation: `remove-start`
fn remove_start(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_mark();
    let _ = editor.remove_start();
    Ok(None)
}

/// Operation: `remove-end`
fn remove_end(env: &mut Environment) -> Result<Option<Action>> {
    let mut editor = env.get_editor().borrow_mut();
    editor.clear_mark();
    let _ = editor.remove_end();
    Ok(None)
}

/// Operation: `copy`
fn copy(env: &mut Environment) -> Result<Option<Action>> {
    let maybe_mark = env.get_editor().borrow_mut().clear_mark();
    let text = if let Some(mark) = maybe_mark {
        env.get_editor().borrow_mut().copy_mark(mark)
    } else {
        env.get_editor().borrow_mut().copy_line()
    };
    env.set_clipboard(text);
    env.get_editor().borrow_mut().render();
    Ok(None)
}

/// Operation: `paste`
fn paste(env: &mut Environment) -> Result<Option<Action>> {
    let maybe_text = env.get_clipboard();
    if let Some(text) = maybe_text {
        env.get_editor().borrow_mut().insert(text);
    }
    Ok(None)
}

/// Operation: `cut`
fn cut(env: &mut Environment) -> Result<Option<Action>> {
    let maybe_mark = env.get_editor().borrow_mut().clear_mark();
    let text = if let Some(mark) = maybe_mark {
        env.get_editor().borrow_mut().remove_mark(mark)
    } else {
        env.get_editor().borrow_mut().remove_line()
    };
    env.set_clipboard(text);
    Ok(None)
}

/// Operation: `open-file`
fn open_file(env: &mut Environment) -> Result<Option<Action>> {
    open_file_at(env, None)
}

/// Operation: `open-file-top`
fn open_file_top(env: &mut Environment) -> Result<Option<Action>> {
    open_file_at(env, Some(Placement::Top))
}

/// Operation: `open-file-bottom`
fn open_file_bottom(env: &mut Environment) -> Result<Option<Action>> {
    open_file_at(env, Some(Placement::Bottom))
}

/// Operation: `open-file-above`
fn open_file_above(env: &mut Environment) -> Result<Option<Action>> {
    open_file_at(env, Some(Placement::Above(env.get_active())))
}

/// Operation: `open-file-below`
fn open_file_below(env: &mut Environment) -> Result<Option<Action>> {
    open_file_at(env, Some(Placement::Below(env.get_active())))
}

/// Various `open-file-*` operations delegate to this functions, but provide `place`
/// to determine the placement of the new window.
fn open_file_at(_: &mut Environment, place: Option<Placement>) -> Result<Option<Action>> {
    let answer_fn = move |env: &mut Environment, answer: Option<&str>| {
        let action = if let Some(path) = answer {
            match open_editor(&path) {
                Ok(editor) => {
                    if let Some(place) = place {
                        if let Some((view_id, _)) = env.open_editor(editor, place, Align::Auto) {
                            env.set_active(Focus::To(view_id));
                            None
                        } else {
                            Action::as_echo(ECHO_CREATE_WINDOW_REFUSED)
                        }
                    } else {
                        env.set_editor(editor, Align::Auto);
                        None
                    }
                }
                Err(e) => Action::as_echo_error(&e),
            }
        } else {
            None
        };
        Ok(action)
    };
    Ok(Action::as_question(
        PROMPT_OPEN_FILE,
        answer_fn,
        complete::file,
    ))
}

/// Operation: `save-file`
fn save_file(env: &mut Environment) -> Result<Option<Action>> {
    let editor = env.get_editor();
    let action = if is_persistent(editor) {
        match stale_editor(editor) {
            Ok(true) => {
                let path = path_of(editor);
                Action::as_question(
                    &prompt_save_anyway(&path),
                    save_override_answer,
                    complete::yes_no,
                )
            }
            Ok(false) => {
                if let Err(e) = save_editor(editor) {
                    Action::as_echo_error(&e)
                } else {
                    let path = path_of(editor);
                    Action::as_echo(&echo_saved(&path))
                }
            }
            Err(e) => Action::as_echo_error(&e),
        }
    } else {
        Action::as_question(PROMPT_SAVE_AS, save_transient_answer, complete::file)
    };
    Ok(action)
}

/// Question callback when requesting override to save an editor.
fn save_override_answer(env: &mut Environment, answer: Option<&str>) -> Result<Option<Action>> {
    let action = match answer {
        Some(yes_no) if yes_no == "y" => {
            let editor = env.get_editor();
            if let Err(e) = save_editor(editor) {
                Action::as_echo_error(&e)
            } else {
                let path = path_of(editor);
                Action::as_echo(&echo_saved(&path))
            }
        }
        Some(yes_no) if yes_no == "n" => None,
        Some(_) => {
            let path = path_of(env.get_editor());
            Action::as_question(
                &prompt_save_anyway(&path),
                save_override_answer,
                complete::yes_no,
            )
        }
        None => None,
    };
    Ok(action)
}

/// Question callback when saving transient editors.
fn save_transient_answer(env: &mut Environment, answer: Option<&str>) -> Result<Option<Action>> {
    let action = if let Some(path) = answer {
        let editor = env.get_editor();
        let time = write_editor(editor, path);
        match time {
            Ok(time) => {
                // Clone transient editor into persistent editor, ensuring that cursor
                // position in buffer and cursor location are preserved.
                let mut buffer = editor.borrow().buffer().clone();
                let cur_pos = editor.borrow().cursor_pos();
                buffer.set_pos(cur_pos);
                let Point { row, col: _ } = editor.borrow().cursor();
                let dup_editor = editor::persistent(path, Some(time), Some(buffer));

                // Replace transient editor in current window with new editor.
                env.set_editor(dup_editor, Align::Row(row));
                Action::as_echo(&echo_saved(path))
            }
            Err(e) => Action::as_echo_error(&e),
        }
    } else {
        None
    };
    Ok(action)
}

/// Operation: `save-file-as`
fn save_file_as(_: &mut Environment) -> Result<Option<Action>> {
    Ok(Action::as_question(
        PROMPT_SAVE_AS,
        save_file_as_answer,
        complete::file,
    ))
}

/// Question callback when saving editor using alternative path.
fn save_file_as_answer(env: &mut Environment, answer: Option<&str>) -> Result<Option<Action>> {
    let action = if let Some(path) = answer {
        let editor = env.get_editor();
        if let Err(e) = save_editor_as(editor, Some(path)) {
            Action::as_echo_error(&e)
        } else {
            Action::as_echo(&echo_saved(&path))
        }
    } else {
        None
    };
    Ok(action)
}

/// Operation: `kill-window`
fn kill_window(env: &mut Environment) -> Result<Option<Action>> {
    let action = if env.view_map().len() > 1 {
        let editor = env.get_editor();
        if is_persistent(editor) && editor.borrow().dirty() {
            let path = path_of(editor);
            Action::as_question(&prompt_save(&path), kill_save_answer, complete::yes_no)
        } else {
            env.kill_window();
            None
        }
    } else {
        Action::as_echo(ECHO_CLOSE_WINDOW_REFUSED)
    };
    Ok(action)
}

fn kill_save_override_answer(
    env: &mut Environment,
    answer: Option<&str>,
) -> Result<Option<Action>> {
    let action = match answer {
        Some(yes_no) if yes_no == "y" => {
            let editor = env.get_editor();
            if let Err(e) = save_editor(editor) {
                Action::as_echo_error(&e)
            } else {
                let path = path_of(editor);
                Action::as_echo(&echo_saved(&path))
            }
        }
        Some(yes_no) if yes_no == "n" => None,
        Some(_) => {
            let path = path_of(env.get_editor());
            Action::as_question(
                &prompt_save_anyway(&path),
                kill_save_override_answer,
                complete::yes_no,
            )
        }
        None => None,
    };
    Ok(action)
}

/// Question callback when killing a window with a dirty buffer.
fn kill_save_answer(env: &mut Environment, answer: Option<&str>) -> Result<Option<Action>> {
    let action = match answer {
        Some(yes_no) if yes_no == "y" => {
            let editor = env.get_editor();
            match stale_editor(editor) {
                Ok(true) => {
                    let path = path_of(editor);
                    Action::as_question(
                        &prompt_save_anyway(&path),
                        kill_save_override_answer,
                        complete::yes_no,
                    )
                }
                Ok(false) => {
                    if let Err(e) = save_editor(editor) {
                        Action::as_echo_error(&e)
                    } else {
                        let path = path_of(editor);
                        env.kill_window();
                        Action::as_echo(&echo_saved(&path))
                    }
                }
                Err(e) => Action::as_echo_error(&e),
            }
        }
        Some(yes_no) if yes_no == "n" => {
            if let Some(_) = env.kill_window() {
                None
            } else {
                Action::as_echo(ECHO_CLOSE_WINDOW_REFUSED)
            }
        }
        Some(_) => {
            let path = path_of(env.get_editor());
            Action::as_question(&prompt_save(&path), kill_save_answer, complete::yes_no)
        }
        None => None,
    };
    Ok(action)
}

/// Operation: `close-window`
fn close_window(env: &mut Environment) -> Result<Option<Action>> {
    let action = if let Some(_) = env.close_window() {
        None
    } else {
        Action::as_echo(ECHO_CLOSE_WINDOW_REFUSED)
    };
    Ok(action)
}

/// Operation: `top-window`
fn top_window(env: &mut Environment) -> Result<Option<Action>> {
    env.set_active(Focus::Top);
    Ok(None)
}

/// Operation: `bottom-window`
fn bottom_window(env: &mut Environment) -> Result<Option<Action>> {
    env.set_active(Focus::Bottom);
    Ok(None)
}

/// Operation: `prev-window`
fn prev_window(env: &mut Environment) -> Result<Option<Action>> {
    env.set_active(Focus::Above);
    Ok(None)
}

/// Operation: `next-window`
fn next_window(env: &mut Environment) -> Result<Option<Action>> {
    env.set_active(Focus::Below);
    Ok(None)
}

/// Operation: `prev-editor`
fn prev_editor(env: &mut Environment) -> Result<Option<Action>> {
    let ids = unattached_editors(env);
    if ids.len() > 0 {
        let editor_id = env.get_editor_id();
        let i = ids.iter().rev().position(|id| *id < editor_id).unwrap_or(0);
        env.switch_editor(ids[ids.len() - i - 1], Align::Auto);
    }
    Ok(None)
}

/// Operation: `next-editor`
fn next_editor(env: &mut Environment) -> Result<Option<Action>> {
    let ids = unattached_editors(env);
    if ids.len() > 0 {
        let editor_id = env.get_editor_id();
        let i = ids.iter().position(|id| *id > editor_id).unwrap_or(0);
        env.switch_editor(ids[i], Align::Auto);
    }
    Ok(None)
}

fn list_editors(env: &mut Environment) -> Result<Option<Action>> {
    let active_id = env.get_active();
    let mut buffer = Buffer::new();
    for (id, e) in env.editor_map() {
        buffer.insert_str(&format!("{id}: {}\n", e.borrow().storage()));
    }
    buffer.set_pos(0);
    let editor = editor::transient("editors", Some(buffer));
    env.open_editor(editor, Placement::Bottom, Align::Auto);
    env.set_active(Focus::To(active_id));
    Ok(None)
}

/// Reads the file at `path` and returns a new editor with the persistent storage type.
pub fn open_editor(path: &str) -> Result<EditorRef> {
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
    let editor = editor::persistent(path, time, Some(buffer));
    Ok(editor)
}

/// Combines [`write_editor`] and [`update_editor`] into a single operation.
fn save_editor(editor: &EditorRef) -> Result<()> {
    save_editor_as(editor, None)
}

/// Combines [`write_editor`] and [`update_editor`] into a single operation, saving
/// the editor using the optional `path`, otherwise it derives the path from `editor`.
fn save_editor_as(editor: &EditorRef, path: Option<&str>) -> Result<()> {
    let path = path
        .map(|path| path.to_string())
        .unwrap_or_else(|| path_of(editor));
    write_editor(editor, &path).map(|time| update_editor(editor, &path, time))
}

/// Writes the buffer of `editor` to `path` and returns the resulting file modification
/// time.
fn write_editor(editor: &EditorRef, path: &str) -> Result<SystemTime> {
    let _ = io::write_file(path, &editor.borrow().buffer())?;
    io::get_time(path)
}

/// Clears the dirty flag on `editor` and sets its storage type to persistent using
/// `path` and the modification `time`.
fn update_editor(editor: &EditorRef, path: &str, time: SystemTime) {
    editor
        .borrow_mut()
        .clear_dirty(Storage::as_persistent(path, Some(time)));
}

/// Returns `true` if `editor` has a modification time older than the modification time
/// of the file in storage.
fn stale_editor(editor: &EditorRef) -> Result<bool> {
    match editor.borrow().storage() {
        Storage::Persistent {
            path,
            time: Some(time),
        } => Ok(io::get_time(&path)? > *time),
        _ => Ok(false),
    }
}

/// Returns an ordered collection of ids for those editors that are *dirty*.
fn dirty_editors(env: &Environment) -> Vec<u32> {
    env.editor_map()
        .iter()
        .filter(|(_, e)| is_persistent(e) && e.borrow().dirty())
        .map(|(id, _)| *id)
        .collect()
}

/// Returns an ordered collection of ids for those editors that are not attached
/// to a window.
fn unattached_editors(env: &Environment) -> Vec<u32> {
    let attached = env.view_map().values().cloned().collect::<Vec<_>>();
    env.editor_map()
        .keys()
        .cloned()
        .filter(|id| !attached.contains(id))
        .collect()
}

/// Returns `true` if `editor` is persistent.
fn is_persistent(editor: &EditorRef) -> bool {
    match editor.borrow().storage() {
        Storage::Persistent { path: _, time: _ } => true,
        _ => false,
    }
}

/// Returns the editor for `editor_id`.
fn get_editor(env: &Environment, editor_id: u32) -> &EditorRef {
    env.editor_map()
        .get(&editor_id)
        .unwrap_or_else(|| panic!("expecting editor id {editor_id}"))
}

/// Returns the path associated with `editor` under the assumption that the storage
/// type is [`Persistent`](Storage::Persistent), otherwise it panics.
fn path_of(editor: &EditorRef) -> String {
    editor
        .borrow()
        .storage()
        .path()
        .unwrap_or_else(|| panic!("path expected for editor"))
}

// This section contains string constants and formatting functions for prompts.
const PROMPT_GOTO_LINE: &str = "goto line:";
const PROMPT_OPEN_FILE: &str = "open file:";
const PROMPT_SAVE_AS: &str = "save as:";

fn prompt_save(path: &str) -> String {
    format!("{path}: save?")
}

fn prompt_save_anyway(path: &str) -> String {
    format!("{path}: file in storage is newer, save anyway?")
}

// This section contains string constants and formatting functions for echoing.
const ECHO_CLOSE_WINDOW_REFUSED: &str = "cannot close only window";
const ECHO_CREATE_WINDOW_REFUSED: &str = "unable to create new window";

fn echo_invalid_line(s: &str) -> String {
    format!("{s}: invalid line number")
}

fn echo_saved(path: &str) -> String {
    format!("{path}: saved")
}

/// Predefined mapping of editing operations to editing functions.
const OP_MAPPINGS: [(&'static str, OpFn); 52] = [
    // --- exit and cancellation ---
    ("quit", quit),
    // --- help ---
    ("help", help),
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
    ("insert-tab", insert_tab),
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
    ("kill-window", kill_window),
    ("close-window", close_window),
    ("top-window", top_window),
    ("bottom-window", bottom_window),
    ("prev-window", prev_window),
    ("next-window", next_window),
    ("prev-editor", prev_editor),
    ("next-editor", next_editor),
    // --- TEMPORARY ---
    ("list-editors", list_editors),
];

pub fn init_op_map() -> OpMap {
    let mut op_map = OpMap::new();
    for (op, op_fn) in OP_MAPPINGS {
        op_map.insert(op, op_fn);
    }
    op_map
}
