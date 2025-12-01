//! A collection of functions intended to be associated with names of editing
//! operations. These functions serve as the glue between [`Key`]s and respective
//! actions in the context of the editing experience.
//!
//! Editing operations are designed to be callable only indirectly through [`OpMap`]
//! instances created by [`init_op_map`]. The mapping of names to functions is captured
//! in [`OP_MAPPINGS`].
//!
//! See [`Bindings`](crate::bind::Bindings) for further details on binding keys
//! at runtime.

use crate::clip::Scope;
use crate::config::ConfigurationRef;
use crate::ed;
use crate::editor::{Align, EditorRef, ImmutableEditor};
use crate::env::{Environment, Focus};
use crate::help;
use crate::operation::{Action, Operation};
use crate::question;
use crate::search::Match;
use crate::size::{Point, Size};
use crate::source::Source;
use crate::workspace::Placement;
use std::collections::HashMap;

/// Operation: `quit`
fn quit(env: &mut Environment) -> Option<Action> {
    question::quit(env)
}

/// Operation: `help`
fn help(env: &mut Environment) -> Option<Action> {
    toggle_help(env, help::HELP_EDITOR_NAME, |config| {
        help::help_editor(config)
    })
}

/// Operation: `help-keys`
fn help_keys(env: &mut Environment) -> Option<Action> {
    toggle_help(env, help::KEYS_EDITOR_NAME, |config| {
        help::keys_editor(config)
    })
}

/// Operation: `help-ops`
fn help_ops(env: &mut Environment) -> Option<Action> {
    toggle_help(env, help::OPS_EDITOR_NAME, |config| {
        help::ops_editor(config)
    })
}

/// Operation: `help-bindings`
fn help_bindings(env: &mut Environment) -> Option<Action> {
    toggle_help(env, help::BINDINGS_EDITOR_NAME, |config| {
        help::bindings_editor(config)
    })
}

/// Operation: `help-colors`
fn help_colors(env: &mut Environment) -> Option<Action> {
    toggle_help(env, help::COLORS_EDITOR_NAME, |config| {
        help::colors_editor(config)
    })
}

/// Operation: `help-syntaxes`
fn help_syntaxes(env: &mut Environment) -> Option<Action> {
    toggle_help(env, help::SYNTAXES_EDITOR_NAME, |config| {
        help::syntaxes_editor(config)
    })
}

fn toggle_help<F>(env: &mut Environment, editor_name: &str, editor_fn: F) -> Option<Action>
where
    F: Fn(ConfigurationRef) -> EditorRef,
{
    let name = Source::as_ephemeral(editor_name).to_string();
    if let Some(editor_id) = env.find_editor_id(&name) {
        if let Some(view_id) = env.find_editor_view_id(editor_id) {
            env.kill_window_for(view_id);
            None
        } else if let Some(view_id) = env.open_window(editor_id, Placement::Bottom, Align::Auto) {
            env.set_active(Focus::To(view_id));
            None
        } else {
            Action::echo("unable to create new window")
        }
    } else {
        let config = env.workspace.borrow().config.clone();
        if let Some((view_id, _)) =
            env.open_editor(editor_fn(config), Placement::Bottom, Align::Auto)
        {
            env.set_active(Focus::To(view_id));
            None
        } else {
            Action::echo("unable to create new window")
        }
    }
}

/// Operation: `move-backward`
fn move_backward(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_backward(1);
    editor.render();
    None
}

/// Operation: `move-backward-word`
fn move_backward_word(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_backward_word();
    editor.render();
    None
}

/// Operation: `move-backward-select`
fn move_backward_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_backward(1);
    editor.render();
    None
}

/// Operation: `move-backward-word-select`
fn move_backward_word_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_backward_word();
    editor.render();
    None
}

/// Operation: `move-forward`
fn move_forward(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_forward(1);
    editor.render();
    None
}

/// Operation: `move-forward-word`
fn move_forward_word(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_forward_word();
    editor.render();
    None
}

/// Operation: `move-forward-select`
fn move_forward_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_forward(1);
    editor.render();
    None
}

/// Operation: `move-forward-word-select`
fn move_forward_word_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_forward_word();
    editor.render();
    None
}

/// Operation: `move-up`
fn move_up(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_up(1, false);
    editor.render();
    None
}

/// Operation: `move-up-select`
fn move_up_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_up(1, false);
    editor.render();
    None
}

/// Operation: `move-down`
fn move_down(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_down(1, false);
    editor.render();
    None
}

/// Operation: `move-down-select`
fn move_down_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_down(1, false);
    editor.render();
    None
}

/// Operation: `move-up-page`
fn move_up_page(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    let rows = editor.rows();
    editor.move_up(rows, true);
    editor.render();
    None
}

/// Operation: `move-up-page-select`
fn move_up_page_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    let rows = editor.rows();
    editor.move_up(rows, true);
    editor.render();
    None
}

/// Operation: `move-down-page`
fn move_down_page(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    let rows = editor.rows();
    editor.move_down(rows, true);
    editor.render();
    None
}

/// Operation: `move-down-page-select`
fn move_down_page_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    let rows = editor.rows();
    editor.move_down(rows, true);
    editor.render();
    None
}

/// Operation: `move-start`
fn move_start(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_start();
    editor.render();
    None
}

/// Operation: `move-start-select`
fn move_start_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_start();
    editor.render();
    None
}

/// Operation: `move-end`
fn move_end(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_end();
    editor.render();
    None
}

/// Operation: `move-end-select`
fn move_end_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_end();
    editor.render();
    None
}

/// Operation: `move-top`
fn move_top(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_top();
    editor.render();
    None
}

/// Operation: `move-top-select`
fn move_top_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_top();
    editor.render();
    None
}

/// Operation: `move-bottom`
fn move_bottom(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.clear_soft_mark();
    editor.move_bottom();
    editor.render();
    None
}

/// Operation: `move-bottom-select`
fn move_bottom_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.move_bottom();
    editor.render();
    None
}

/// Operation: `scroll-up`
fn scroll_up(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();

    // Capture current buffer position before scrolling in case soft mark needs to
    // be cleared.
    let prior_pos = editor.pos();
    editor.scroll_up(1);

    // Clear soft mark if buffer position moved as a result of scrolling.
    if editor.pos() != prior_pos {
        editor.clear_soft_mark();
    }
    editor.render();
    None
}

/// Operation: `scroll-up-select`
fn scroll_up_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.scroll_up(1);
    editor.render();
    None
}

/// Operation: `scroll-down`
fn scroll_down(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();

    // Capture current buffer position before scrolling in case soft mark needs to
    // be cleared.
    let prior_pos = editor.pos();
    editor.scroll_down(1);

    // Clear soft mark if buffer position moved as a result of scrolling.
    if editor.pos() != prior_pos {
        editor.clear_soft_mark();
    }
    editor.render();
    None
}

/// Operation: `scroll-down-select`
fn scroll_down_select(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    editor.set_soft_mark();
    editor.scroll_down(1);
    editor.render();
    None
}

/// Operation: `scroll-center`
fn scroll_center(env: &mut Environment) -> Option<Action> {
    // Rotate through alignment based on current cursor position using following
    // cycle: center -> bottom -> top.
    //
    // If position is not precisely on one of these rows, then start at center. This
    // behavior allows user to quickly align cursor with successive key presses.
    let mut editor = env.get_active_editor().borrow_mut();
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
    editor.render();
    None
}

/// Operation: `redraw`
fn redraw(_: &mut Environment) -> Option<Action> {
    Action::redraw()
}

/// Operation: `set-mark`
fn set_mark(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if editor.clear_mark().is_some() {
        editor.render();
    } else {
        editor.set_hard_mark();
    }
    None
}

/// Operation: `goto-line`
fn goto_line(env: &mut Environment) -> Option<Action> {
    question::goto_line(env.get_active_editor().clone())
}

pub fn insert_char(env: &mut Environment, c: char) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if let Some(editor) = editor.modify() {
        editor.clear_mark();
        editor.insert_char(c);
        editor.render();
        None
    } else {
        Action::echo("editor is readonly")
    }
}

/// Operation: `insert-line`
fn insert_line(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if let Some(editor) = editor.modify() {
        editor.clear_mark();
        editor.insert_char('\n');
        editor.render();
        None
    } else {
        Action::echo("editor is readonly")
    }
}

/// Operation: `insert-tab`
fn insert_tab(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if let Some(editor) = editor.modify() {
        editor.clear_mark();
        editor.insert_tab();
        editor.render();
        None
    } else {
        Action::echo("editor is readonly")
    }
}

/// Operation: `insert-unicode-dec`
fn insert_unicode_dec(env: &mut Environment) -> Option<Action> {
    question::insert_unicode(env.get_active_editor().clone(), 10)
}

/// Operation: `insert-unicode-hex`
fn insert_unicode_hex(env: &mut Environment) -> Option<Action> {
    question::insert_unicode(env.get_active_editor().clone(), 16)
}

/// Operation: `remove-before`
fn remove_before(env: &mut Environment) -> Option<Action> {
    let text = {
        let mut editor = env.get_active_editor().borrow_mut();
        if let Some(editor) = editor.modify() {
            let maybe_mark = editor.clear_mark();
            let text = if let Some(mark) = maybe_mark {
                let text = editor.remove_mark(mark);
                Some(text)
            } else {
                editor.remove_before();
                None
            };
            editor.render();
            text
        } else {
            None
        }
    };
    if let Some(text) = text {
        env.clipboard.set_text(text, Scope::Local);
    }
    None
}

/// Operation: `remove-after`
fn remove_after(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if let Some(editor) = editor.modify() {
        editor.clear_mark();
        editor.remove_after();
        editor.render();
        None
    } else {
        Action::echo("editor is readonly")
    }
}

/// Operation: `remove-start`
fn remove_start(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if let Some(editor) = editor.modify() {
        editor.clear_mark();
        editor.remove_start();
        editor.render();
        None
    } else {
        Action::echo("editor is readonly")
    }
}

/// Operation: `remove-end`
fn remove_end(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if let Some(editor) = editor.modify() {
        editor.clear_mark();
        editor.remove_end();
        editor.render();
        None
    } else {
        Action::echo("editor is readonly")
    }
}

/// Operation: `undo`
fn undo(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if editor.undo() {
        editor.render();
        None
    } else {
        Action::echo("nothing to undo")
    }
}

/// Operation: `redo`
fn redo(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if editor.redo() {
        editor.render();
        None
    } else {
        Action::echo("nothing to redo")
    }
}

/// Operation: `copy`
fn copy(env: &mut Environment) -> Option<Action> {
    copy_to(env, Scope::Local)
}

/// Operation: `copy-global`
fn copy_global(env: &mut Environment) -> Option<Action> {
    copy_to(env, Scope::Global)
}

fn copy_to(env: &mut Environment, scope: Scope) -> Option<Action> {
    let text = {
        let mut editor = env.get_active_editor().borrow_mut();
        let maybe_mark = editor.clear_mark();
        if let Some(mark) = maybe_mark {
            editor.copy_mark(mark)
        } else {
            editor.copy_line()
        }
    };
    env.get_active_editor().borrow_mut().render();
    env.clipboard.set_text(text, scope);
    None
}

/// Operation: `paste`
fn paste(env: &mut Environment) -> Option<Action> {
    paste_from(env, Scope::Local)
}

/// Operation: `paste-global`
fn paste_global(env: &mut Environment) -> Option<Action> {
    paste_from(env, Scope::Global)
}

fn paste_from(env: &mut Environment, scope: Scope) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if let Some(editor) = editor.modify() {
        let maybe_text = env.clipboard.get_text(scope);
        if let Some(text) = maybe_text {
            editor.insert(&text);
            editor.render();
        }
        None
    } else {
        Action::echo("editor is readonly")
    }
}

/// Operation: `cut`
fn cut(env: &mut Environment) -> Option<Action> {
    cut_to(env, Scope::Local)
}

/// Operation: `cut-global`
fn cut_global(env: &mut Environment) -> Option<Action> {
    cut_to(env, Scope::Global)
}

fn cut_to(env: &mut Environment, scope: Scope) -> Option<Action> {
    let text = {
        let mut editor = env.get_active_editor().borrow_mut();
        if let Some(editor) = editor.modify() {
            let text = {
                let maybe_mark = editor.clear_mark();
                if let Some(mark) = maybe_mark {
                    editor.remove_mark(mark)
                } else {
                    editor.remove_line()
                }
            };
            editor.render();
            Some(text)
        } else {
            None
        }
    };
    if let Some(text) = text {
        env.clipboard.set_text(text, scope);
        None
    } else {
        Action::echo("editor is readonly")
    }
}

/// Operation: `search`
fn search(env: &mut Environment) -> Option<Action> {
    question::search_term(env.get_active_editor().clone(), false)
}

/// Operation: `search-case`
fn search_case(env: &mut Environment) -> Option<Action> {
    question::search_term(env.get_active_editor().clone(), true)
}

/// Operation: `search-regex`
fn search_regex(env: &mut Environment) -> Option<Action> {
    question::search_regex(env.get_active_editor().clone(), false)
}

/// Operation: `search-regex-case`
fn search_regex_case(env: &mut Environment) -> Option<Action> {
    question::search_regex(env.get_active_editor().clone(), true)
}

/// Operation: `search-next`
fn search_next(env: &mut Environment) -> Option<Action> {
    let editor = env.get_active_editor().clone();
    let last_match = editor.borrow_mut().take_last_match();
    if let Some((pos, pattern)) = last_match {
        // If position of last match is also current buffer position, then advance
        // to next position before resuming search.
        let mut editor = editor.borrow_mut();
        let cur_pos = editor.pos();
        let pos = if pos == cur_pos { cur_pos + 1 } else { cur_pos };

        // Find next match and highlight if found.
        let result = pattern.find(&editor.buffer(), pos);
        match result {
            Some(Match(start_pos, end_pos)) => {
                editor.move_to(start_pos, Align::Center);
                editor.clear_mark();
                editor.set_soft_mark_at(end_pos);
                editor.render();
                editor.set_last_match(start_pos, pattern);
            }
            None => {
                // Restore match state that was taken earlier.
                editor.set_last_match(pos, pattern);
            }
        }
        None
    } else {
        // Since no prior match exists, act as if new term search is started.
        question::search_term(editor, false)
    }
}

/// Operation: `open-file`
fn open_file(env: &mut Environment) -> Option<Action> {
    question::open(ed::derive_dir(env), None)
}

/// Operation: `open-file-top`
fn open_file_top(env: &mut Environment) -> Option<Action> {
    question::open(ed::derive_dir(env), Some(Placement::Top))
}

/// Operation: `open-file-bottom`
fn open_file_bottom(env: &mut Environment) -> Option<Action> {
    question::open(ed::derive_dir(env), Some(Placement::Bottom))
}

/// Operation: `open-file-above`
fn open_file_above(env: &mut Environment) -> Option<Action> {
    question::open(
        ed::derive_dir(env),
        Some(Placement::Above(env.get_active_view_id())),
    )
}

/// Operation: `open-file-below`
fn open_file_below(env: &mut Environment) -> Option<Action> {
    question::open(
        ed::derive_dir(env),
        Some(Placement::Below(env.get_active_view_id())),
    )
}

/// Operation: `save-file`
fn save_file(env: &mut Environment) -> Option<Action> {
    let editor = env.get_active_editor();
    if ed::is_file(editor) {
        match ed::stale_editor(editor) {
            Ok(true) => question::save_override(editor.clone()),
            Ok(false) => question::save_now(editor),
            Err(e) => Action::echo(&e),
        }
    } else {
        question::save(editor.clone())
    }
}

/// Operation: `save-file-as`
fn save_file_as(env: &mut Environment) -> Option<Action> {
    question::save(env.get_active_editor().clone())
}

/// Operation: `kill-window`
fn kill_window(env: &mut Environment) -> Option<Action> {
    if env.view_map().len() > 1 {
        let editor = env.get_active_editor();
        if ed::is_dirty_file(editor) {
            question::kill(editor.clone(), None)
        } else {
            env.kill_window();
            None
        }
    } else if let Some((switch_id, _)) = ed::next_unattached_editor(env) {
        let editor_id = env.get_active_editor_id();
        let editor = env.get_active_editor();
        if ed::is_dirty_file(editor) {
            question::kill(editor.clone(), Some((editor_id, switch_id)))
        } else {
            env.switch_editor(switch_id, Align::Auto);
            env.close_editor(editor_id);
            None
        }
    } else {
        Action::echo("cannot close only window")
    }
}

/// Operation: `close-window`
fn close_window(env: &mut Environment) -> Option<Action> {
    if env.close_window().is_some() {
        None
    } else {
        Action::echo("cannot close only window")
    }
}

/// Operation: `close-other-windows`
fn close_other_windows(env: &mut Environment) -> Option<Action> {
    let active_id = env.get_active_view_id();
    let other_ids = env
        .view_map()
        .keys()
        .cloned()
        .filter(|id| *id != active_id)
        .collect::<Vec<_>>();
    for id in other_ids {
        env.close_window_for(id);
    }
    None
}

/// Operation: `top-window`
fn top_window(env: &mut Environment) -> Option<Action> {
    env.set_active(Focus::Top);
    None
}

/// Operation: `bottom-window`
fn bottom_window(env: &mut Environment) -> Option<Action> {
    env.set_active(Focus::Bottom);
    None
}

/// Operation: `prev-window`
fn prev_window(env: &mut Environment) -> Option<Action> {
    env.set_active(Focus::Above);
    None
}

/// Operation: `next-window`
fn next_window(env: &mut Environment) -> Option<Action> {
    env.set_active(Focus::Below);
    None
}

/// Operation: `select-editor`
fn select_editor(env: &mut Environment) -> Option<Action> {
    let editors = ed::unattached_editors(env, true);
    if editors.len() > 0 {
        question::select(editors, None)
    } else {
        Action::echo("no more editors")
    }
}

/// Operation: `select-editor-top`
fn select_editor_top(env: &mut Environment) -> Option<Action> {
    let editors = ed::unattached_editors(env, true);
    if editors.len() > 0 {
        question::select(editors, Some(Placement::Top))
    } else {
        Action::echo("no more editors")
    }
}

/// Operation: `select-editor-bottom`
fn select_editor_bottom(env: &mut Environment) -> Option<Action> {
    let editors = ed::unattached_editors(env, true);
    if editors.len() > 0 {
        question::select(editors, Some(Placement::Bottom))
    } else {
        Action::echo("no more editors")
    }
}

/// Operation: `select-editor-above`
fn select_editor_above(env: &mut Environment) -> Option<Action> {
    let editors = ed::unattached_editors(env, true);
    if editors.len() > 0 {
        question::select(editors, Some(Placement::Above(env.get_active_view_id())))
    } else {
        Action::echo("no more editors")
    }
}

/// Operation: `select-editor-below`
fn select_editor_below(env: &mut Environment) -> Option<Action> {
    let editors = ed::unattached_editors(env, true);
    if editors.len() > 0 {
        question::select(editors, Some(Placement::Below(env.get_active_view_id())))
    } else {
        Action::echo("no more editors")
    }
}

/// Operation: `prev-editor`
fn prev_editor(env: &mut Environment) -> Option<Action> {
    if let Some((prev_id, _)) = ed::prev_unattached_editor(env) {
        env.switch_editor(prev_id, Align::Auto);
    }
    None
}

/// Operation: `next-editor`
fn next_editor(env: &mut Environment) -> Option<Action> {
    if let Some((next_id, _)) = ed::next_unattached_editor(env) {
        env.switch_editor(next_id, Align::Auto);
    }
    None
}

/// Operation: `describe-editor`
fn describe_editor(env: &mut Environment) -> Option<Action> {
    let editor = env.get_active_editor().borrow();
    let buffer = editor.buffer();
    let (c_char, c_code) = if let Some(c) = buffer.get_char(editor.pos()) {
        let c_char = if c.is_control() {
            "".to_string()
        } else {
            format!("'{c}' ")
        };
        (c_char, format!("\\u{:04x}", c as u32))
    } else {
        ("EOF".to_string(), "".to_string())
    };
    let tab_mode = if editor.get_tab() { "hard" } else { "soft" };
    let eol_mode = if editor.get_crlf() { "CRLF" } else { "LF" };
    let text = format!(
        "lines: {} | chars: {} | cursor: {c_char}{c_code} | tabs: {tab_mode} | eol: {eol_mode}",
        buffer.line_of(usize::MAX) + 1,
        buffer.size(),
    );
    Action::echo(&text)
}

/// Operation: `tab-mode`
fn tab_mode(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    let hard = editor.get_tab();
    editor.set_tab(!hard);
    if hard {
        Action::echo("soft tabs enabled")
    } else {
        Action::echo("hard tabs enabled")
    }
}

/// Operation: `eol-mode`
fn eol_mode(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    let crlf = editor.get_crlf();
    editor.set_crlf(!crlf);
    if crlf {
        Action::echo("EOL mode set to LF")
    } else {
        Action::echo("EOL mode set to CRLF")
    }
}

/// Scrolls the display down for the editor associated with `p`, which represents a
/// point whose origin is the top-left position of the terminal display.
pub fn track_up(env: &mut Environment, p: Point, select: bool) {
    let view = env.workspace.borrow().locate_view(p);
    if let Some((view_id, _)) = view {
        let mut editor = env.get_view_editor(view_id).borrow_mut();

        // Update soft mark if selection is active, otherwise capture current buffer
        // position before scrolling in case soft mark needs to be cleared.
        let prior_pos = if select {
            editor.set_soft_mark();
            None
        } else {
            Some(editor.pos())
        };
        editor.scroll_down(1);

        // If selection is inactive and buffer position moved as a result of scrolling,
        // then soft mark must be cleared.
        if let Some(prior_pos) = prior_pos
            && editor.pos() != prior_pos
        {
            editor.clear_soft_mark();
        }
        editor.render();
    }
}

/// Scrolls the display up for the editor associated with `p`, which represents a
/// point whose origin is the top-left position of the terminal display.
pub fn track_down(env: &mut Environment, p: Point, select: bool) {
    let view = env.workspace.borrow().locate_view(p);
    if let Some((view_id, _)) = view {
        let mut editor = env.get_view_editor(view_id).borrow_mut();

        // Update soft mark if selection is active, otherwise capture current buffer
        // position before scrolling in case soft mark needs to be cleared.
        let prior_pos = if select {
            editor.set_soft_mark();
            None
        } else {
            Some(editor.pos())
        };
        editor.scroll_up(1);

        // If selection is inactive and buffer position moved as a result of scrolling,
        // then soft mark must be cleared.
        if let Some(prior_pos) = prior_pos
            && editor.pos() != prior_pos
        {
            editor.clear_soft_mark();
        }
        editor.render();
    }
}

/// Moves the cursor backward for the editor associated with `p`, which represents a
/// point whose origin is the top-left position of the terminal display.
pub fn track_backward(env: &mut Environment, p: Point, select: bool) {
    let ws = env.workspace.borrow();
    if ws.config.settings.track_lateral {
        let view = ws.locate_view(p);
        if let Some((view_id, _)) = view {
            let mut editor = env.get_view_editor(view_id).borrow_mut();
            if select {
                editor.set_soft_mark();
            } else {
                editor.clear_soft_mark();
            }
            editor.move_backward(1);
            editor.render();
        }
    }
}

/// Moves the cursor forward for the editor associated with `p`, which represents a
/// point whose origin is the top-left position of the terminal display.
pub fn track_forward(env: &mut Environment, p: Point, select: bool) {
    let ws = env.workspace.borrow();
    if ws.config.settings.track_lateral {
        let view = ws.locate_view(p);
        if let Some((view_id, _)) = view {
            let mut editor = env.get_view_editor(view_id).borrow_mut();
            if select {
                editor.set_soft_mark();
            } else {
                editor.clear_soft_mark();
            }
            editor.move_forward(1);
            editor.render();
        }
    }
}

/// Sets the active editor and cursor position within editor based on `p`, which
/// represents a point whose origin is the top-left position of the terminal display.
pub fn set_focus(env: &mut Environment, p: Point) {
    let view = env.workspace.borrow().locate_view(p);
    if let Some((view_id, cursor)) = view {
        env.set_active(Focus::To(view_id));
        let mut editor = env.get_active_editor().borrow_mut();
        editor.clear_soft_mark();
        editor.set_focus(cursor);
        editor.render();
    }
}

/// Predefined mapping of editing operations to editing functions.
#[rustfmt::skip]
pub const OP_MAPPINGS: [(&str, Operation, &str); 84] = [
    // --- exit and cancellation ---
    ("quit", quit,
        "ask to save dirty editors and quit"),

    // --- help ---
    ("help", help,
        "show or hide @help window"),
    ("help-keys", help_keys,
        "show or hide @keys window"),
    ("help-ops", help_ops,
        "show or hide @operations window"),
    ("help-bindings", help_bindings,
        "show or hide @bindings window"),
    ("help-colors", help_colors,
        "show or hide @colors window"),
    ("help-syntaxes", help_syntaxes,
        "show or hide @syntaxes window"),

    // --- navigation and selection ---
    ("move-backward", move_backward,
        "move cursor backward one character"),
    ("move-backward-word", move_backward_word,
        "move cursor backward one word"),
    ("move-backward-select", move_backward_select,
        "move cursor backward one character while selecting text"),
    ("move-backward-word-select", move_backward_word_select,
        "move cursor backward one word while selecting text"),
    ("move-forward", move_forward,
        "move cursor forward one character"),
    ("move-forward-word", move_forward_word,
        "move cursor forward one word"),
    ("move-forward-select", move_forward_select,
        "move cursor forward one character while selecting text"),
    ("move-forward-word-select", move_forward_word_select,
        "move cursor forward one word while selecting text"),
    ("move-up", move_up,
        "move cursor up one line"),
    ("move-up-select", move_up_select,
        "move cursor up one line while selecting text"),
    ("move-down", move_down,
        "move cursor down one line"),
    ("move-down-select", move_down_select,
        "move cursor down one line while selecting text"),
    ("move-up-page", move_up_page,
        "move cursor up one page"),
    ("move-up-page-select", move_up_page_select,
        "move cursor up one page while selecting text"),
    ("move-down-page", move_down_page,
        "move cursor down one page"),
    ("move-down-page-select", move_down_page_select,
        "move cursor down one page while selecting text"),
    ("move-start", move_start,
        "move cursor to start of line"),
    ("move-start-select", move_start_select,
        "move cursor to start of line while selecting text"),
    ("move-end", move_end,
        "move cursor to end of line"),
    ("move-end-select", move_end_select,
        "move cursor to end of line while selecting text"),
    ("move-top", move_top,
        "move cursor to top of buffer"),
    ("move-top-select", move_top_select,
        "move cursor to top of buffer while selecting text"),
    ("move-bottom", move_bottom,
        "move cursor to bottom of buffer"),
    ("move-bottom-select", move_bottom_select,
        "move cursor to bottom of buffer while selecting text"),
    ("scroll-up", scroll_up,
        "scroll contents of window up one line"),
    ("scroll-up-select", scroll_up_select,
        "scroll contents of window up one line while selecting text"),
    ("scroll-down", scroll_down,
        "scroll contents of window down one line"),
    ("scroll-down-select", scroll_down_select,
        "scroll contents of window down one line while selecting text"),
    ("scroll-center", scroll_center,
        "redraw window and align cursor to center, then bottom and top when repeated"),
    ("redraw", redraw,
        "redraw entire workspace"),
    ("set-mark", set_mark,
        "set or unset mark for selecting text"),
    ("goto-line", goto_line,
        "move cursor to line"),

    // --- insertion and removal ---
    ("insert-line", insert_line,
        "insert line break"),
    ("insert-tab", insert_tab,
        "insert soft or hard tab"),
    ("insert-unicode-dec", insert_unicode_dec,
        "insert unicode character as decimal code point"),
    ("insert-unicode-hex", insert_unicode_hex,
        "insert unicode character as hex code point"),
    ("remove-before", remove_before,
        "remove character before cursor"),
    ("remove-after", remove_after,
        "remove character after cursor"),
    ("remove-start", remove_start,
        "remove characters from start of line to cursor"),
    ("remove-end", remove_end,
        "remove characters from cursor to end of line"),
    ("undo", undo,
        "undo last change"),
    ("redo", redo,
        "reapply last undo"),

    // --- selection actions ---
    ("copy", copy,
        "copy selection or entire line if nothing selected to local clipboard"),
    ("copy-global", copy_global,
        "copy selection or entire line if nothing selected to global clipboard"),
    ("paste", paste,
        "paste contents of local clipboard"),
    ("paste-global", paste_global,
        "paste contents of global clipboard"),
    ("cut", cut,
        "cut and copy selection or entire line if nothing selected to local clipboard"),
    ("cut-global", cut_global,
        "cut and copy selection or entire line if nothing selected to global clipboard"),

    // --- search ---
    ("search", search,
        "case-insensitive search using term"),
    ("search-case", search_case,
        "case-sensitive search using term"),
    ("search-regex", search_regex,
        "case-insensitive search using regular expression"),
    ("search-regex-case", search_regex_case,
        "case-sensitive search using regular expression"),
    ("search-next", search_next,
        "search for next match"),

    // --- file handling ---
    ("open-file", open_file,
        "open file in current window"),
    ("open-file-top", open_file_top,
        "open file in new window at top of workspace"),
    ("open-file-bottom", open_file_bottom,
        "open file in new window at bottom of workspace"),
    ("open-file-above", open_file_above,
        "open file in new window above current window"),
    ("open-file-below", open_file_below,
        "open file in new window below current window"),
    ("save-file", save_file,
        "save file"),
    ("save-file-as", save_file_as,
        "save file as another name"),

    // --- editor handling ---
    ("select-editor", select_editor,
        "switch to editor in current window"),
    ("select-editor-top", select_editor_top,
        "switch to editor in new window at top of workspace"),
    ("select-editor-bottom", select_editor_bottom,
        "switch to editor in new window at bottom of workspace"),
    ("select-editor-above", select_editor_above,
        "switch to editor in new window above current window"),
    ("select-editor-below", select_editor_below,
        "switch to editor in new window below current window"),
    ("prev-editor", prev_editor,
        "switch to previous editor in current window"),
    ("next-editor", next_editor,
        "switch to next editor in current window"),

    // --- window handling ---
    ("kill-window", kill_window,
        "close current window and editor"),
    ("close-window", close_window,
        "close current window"),
    ("close-other-windows", close_other_windows,
        "close all windows other than current window"),
    ("top-window", top_window,
        "move to window at top of workspace"),
    ("bottom-window", bottom_window,
        "move to window at bottom of workspace"),
    ("prev-window", prev_window,
        "move to window above current window"),
    ("next-window", next_window,
        "move to window below current window"),

    // --- behaviors ---
    ("describe-editor", describe_editor,
        "show editor information"),
    ("tab-mode", tab_mode,
        "toggle between soft and hard tab insertion mode"),
    ("eol-mode", eol_mode,
        "toggle between CRLF and LF when saving files"),
];

/// Returns a mapping of editing operations to editing functions.
pub fn init_op_map() -> HashMap<&'static str, Operation> {
    let mut op_map = HashMap::new();
    for (op, op_fn, _) in OP_MAPPINGS {
        op_map.insert(op, op_fn);
    }
    op_map
}

/// Returns a description of `op`.
pub fn describe(op: &str) -> Option<&'static str> {
    OP_MAPPINGS
        .iter()
        .find(|(name, _, _)| *name == op)
        .map(|(_, _, desc)| *desc)
}
