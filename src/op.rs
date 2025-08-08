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

use crate::buffer::Buffer;
use crate::clip::Scope;
use crate::config::ConfigurationRef;
use crate::editor::{Align, Capture, Editor, EditorRef, ImmutableEditor};
use crate::env::{Environment, Focus};
use crate::error::{Error, Result};
use crate::help;
use crate::io;
use crate::key::{Key, TAB};
use crate::search::{self, Pattern};
use crate::size::{Point, Size};
use crate::source::Source;
use crate::sys::{self, AsString};
use crate::user::{self, Completer, Inquirer};
use crate::workspace::Placement;
use regex_lite::RegexBuilder;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A function type that implements an editing operation.
pub type OpFn = fn(&mut Environment) -> Option<Action>;

/// Map of canonical names to editing operations.
pub type OpMap = HashMap<&'static str, OpFn>;

/// An action returned by an [`OpFn`] that is carried out by a controller orchestrating
/// calls to such functions.
pub enum Action {
    Quit,
    Redraw,
    Echo(String),
    Question(Box<dyn Inquirer>),
}

impl Action {
    fn as_quit() -> Option<Action> {
        Some(Action::Quit)
    }

    fn as_redraw() -> Option<Action> {
        Some(Action::Redraw)
    }

    fn as_echo<T: ToString + ?Sized>(text: &T) -> Option<Action> {
        let action = Action::Echo(text.to_string());
        Some(action)
    }

    fn as_question(inquirer: Box<dyn Inquirer>) -> Option<Action> {
        let action = Action::Question(inquirer);
        Some(action)
    }

    fn echo_readonly() -> Option<Action> {
        Self::as_echo("editor is readonly")
    }

    fn echo_no_window() -> Option<Action> {
        Self::as_echo("unable to create new window")
    }

    fn echo_cannot_close() -> Option<Action> {
        Self::as_echo("cannot close only window")
    }

    fn echo_no_editors() -> Option<Action> {
        Self::as_echo("no more editors")
    }
}

/// Operation: `quit`
fn quit(env: &mut Environment) -> Option<Action> {
    Quit::start(env)
}

/// An inquirer that orchestrates the _quit_ process, which may involve saving dirty
/// editors.
struct Quit {
    /// List of dirty editors.
    dirty: Vec<EditorRef>,
}

impl Quit {
    /// Starts the process of saving dirty editors before quitting.
    fn start(env: &Environment) -> Option<Action> {
        let dirty = dirty_editors(env);
        if dirty.len() > 0 {
            Action::as_question(Quit { dirty }.into_box())
        } else {
            Action::as_quit()
        }
    }

    /// Continues the process of saving editors if `dirty` is not empty.
    fn next(dirty: &[EditorRef]) -> Option<Action> {
        if dirty.len() > 1 {
            let dirty = dirty[1..].to_vec();
            Action::as_question(Quit { dirty }.into_box())
        } else {
            Action::as_quit()
        }
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    /// Saves the first dirty editor and then continues to the next editor.
    fn save_first(&mut self) -> Option<Action> {
        let editor = &self.dirty[0];
        match stale_editor(editor) {
            Ok(true) => QuitOverride::question(self.dirty.clone()),
            Ok(false) => {
                if let Err(e) = save_editor(editor) {
                    Action::as_echo(&e)
                } else {
                    Self::next(&self.dirty)
                }
            }
            Err(e) => Action::as_echo(&e),
        }
    }

    /// Saves all dirty editors.
    fn save_all(&mut self) -> Option<Action> {
        let mut dirty_iter = self.dirty.iter();
        while let Some(editor) = dirty_iter.next() {
            match stale_editor(editor) {
                Ok(true) => {
                    let mut dirty = vec![editor.clone()];
                    dirty.extend(dirty_iter.cloned());
                    return QuitOverride::question(dirty);
                }
                Ok(false) => {
                    if let Err(e) = save_editor(editor) {
                        return Action::as_echo(&e);
                    }
                }
                Err(e) => {
                    return Action::as_echo(&e);
                }
            }
        }
        Action::as_quit()
    }
}

impl Inquirer for Quit {
    fn prompt(&self) -> String {
        let source = source_of(&self.dirty[0]);
        format!("{source}: save?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_all_completer()
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => self.save_first(),
            Some("a") => self.save_all(),
            Some("n") => Self::next(&self.dirty),
            Some(_) => Self::start(env),
            None => None,
        }
    }
}

/// An inquirer spawned from [`Quit`] that orchestrates the saving of an editor whose
/// corresponding file in storage is newer than its timestamp.
#[derive(Clone)]
struct QuitOverride {
    /// List of dirty editors, where the first entry is pertinent to this flow.
    dirty: Vec<EditorRef>,
}

impl QuitOverride {
    fn question(dirty: Vec<EditorRef>) -> Option<Action> {
        Action::as_question(QuitOverride { dirty }.into_box())
    }

    fn again(&self) -> Option<Action> {
        Action::as_question(self.clone().into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    fn save(&mut self) -> Option<Action> {
        if let Err(e) = save_editor(&self.dirty[0]) {
            Action::as_echo(&e)
        } else {
            Quit::next(&self.dirty)
        }
    }
}

impl Inquirer for QuitOverride {
    fn prompt(&self) -> String {
        let source = source_of(&self.dirty[0]);
        format!("{source}: file in storage is newer, save anyway?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => self.save(),
            Some("n") => Quit::next(&self.dirty),
            Some(_) => self.again(),
            None => None,
        }
    }
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
            Action::echo_no_window()
        }
    } else {
        let config = env.workspace.borrow().config.clone();
        if let Some((view_id, _)) =
            env.open_editor(editor_fn(config), Placement::Bottom, Align::Auto)
        {
            env.set_active(Focus::To(view_id));
            None
        } else {
            Action::echo_no_window()
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
    Action::as_redraw()
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
    GotoLine::question(env.get_active_editor().clone())
}

/// An inquirer that orchestrates going to a specific line in an editor.
struct GotoLine {
    editor: EditorRef,
    capture: Capture,
}

impl GotoLine {
    const PROMPT: &str = "goto line:";
    const INVALID_HINT: &str = " (invalid)";

    fn question(editor: EditorRef) -> Option<Action> {
        let capture = editor.borrow().capture();
        Action::as_question(GotoLine { editor, capture }.into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    fn restore(&mut self) {
        let mut editor = self.editor.borrow_mut();
        editor.restore(&self.capture);
        editor.render();
    }
}

impl Inquirer for GotoLine {
    fn prompt(&self) -> String {
        Self::PROMPT.to_string()
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::number_completer(10)
    }

    fn react(&mut self, _: &mut Environment, value: &str, _: &Key) -> Option<String> {
        let value = value.trim();
        if value.len() > 0 {
            if let Ok(line) = value.parse::<u32>() {
                let line = if line > 0 { line - 1 } else { 0 };
                let mut editor = self.editor.borrow_mut();
                editor.move_line(line, Align::Center);
                editor.render();
                None
            } else {
                Some(Self::INVALID_HINT.to_string())
            }
        } else {
            self.restore();
            None
        }
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        if value.is_none() {
            self.restore();
        }
        None
    }
}

pub fn insert_char(env: &mut Environment, c: char) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if let Some(editor) = editor.modify() {
        editor.clear_mark();
        editor.insert_char(c);
        editor.render();
        None
    } else {
        Action::echo_readonly()
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
        Action::echo_readonly()
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
        Action::echo_readonly()
    }
}

/// Operation: `insert-unicode-dec`
fn insert_unicode_dec(_: &mut Environment) -> Option<Action> {
    InsertUnicode::question(10)
}

/// Operation: `insert-unicode-hex`
fn insert_unicode_hex(_: &mut Environment) -> Option<Action> {
    InsertUnicode::question(16)
}

/// An inquirer that inserts a Unicode character.
struct InsertUnicode {
    /// Only values of `10` and `16` are supported.
    radix: u32,
}

impl InsertUnicode {
    const INVALID_HINT: &str = " (invalid)";

    fn question(radix: u32) -> Option<Action> {
        debug_assert!(radix == 10 || radix == 16);
        Action::as_question(InsertUnicode { radix }.into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    fn parse_code(&self, value: &str) -> Option<char> {
        u32::from_str_radix(value, self.radix)
            .ok()
            .and_then(char::from_u32)
    }
}

impl Inquirer for InsertUnicode {
    fn prompt(&self) -> String {
        let radix = if self.radix == 10 { "" } else { " (hex)" };
        format!("insert code point{radix}:")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::number_completer(self.radix)
    }

    fn react(&mut self, _: &mut Environment, value: &str, _: &Key) -> Option<String> {
        let value = value.trim();
        if value.len() > 0 {
            if let Some(c) = self.parse_code(value) {
                if c.is_control() {
                    None
                } else {
                    Some(format!(" '{c}'"))
                }
            } else {
                Some(Self::INVALID_HINT.to_string())
            }
        } else {
            None
        }
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(value) = value
            && let Some(c) = self.parse_code(value)
        {
            let mut editor = env.get_active_editor().borrow_mut();
            if let Some(editor) = editor.modify() {
                editor.clear_mark();
                editor.insert_char(c);
                editor.render();
                None
            } else {
                Action::echo_readonly()
            }
        } else {
            None
        }
    }
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
        Action::echo_readonly()
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
        Action::echo_readonly()
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
        Action::echo_readonly()
    }
}

/// Operation: `undo`
fn undo(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if editor.undo() {
        editor.render();
        None
    } else {
        Action::as_echo("nothing to undo")
    }
}

/// Operation: `redo`
fn redo(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    if editor.redo() {
        editor.render();
        None
    } else {
        Action::as_echo("nothing to redo")
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
        Action::echo_readonly()
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
        Action::echo_readonly()
    }
}

/// Operation: `search`
fn search(env: &mut Environment) -> Option<Action> {
    Search::question(env.get_active_editor().clone(), false, false)
}

/// Operation: `search-case`
fn search_case(env: &mut Environment) -> Option<Action> {
    Search::question(env.get_active_editor().clone(), false, true)
}

/// Operation: `search-regex`
fn search_regex(env: &mut Environment) -> Option<Action> {
    Search::question(env.get_active_editor().clone(), true, false)
}

/// Operation: `search-regex-case`
fn search_regex_case(env: &mut Environment) -> Option<Action> {
    Search::question(env.get_active_editor().clone(), true, true)
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
        let found = pattern.find(&editor.buffer(), pos);
        if let Some((start_pos, end_pos)) = found {
            editor.move_to(start_pos, Align::Center);
            editor.clear_mark();
            editor.set_soft_mark_at(end_pos);
            editor.render();
            editor.set_last_match(start_pos, pattern);
        } else {
            // Restore match state that was taken earlier.
            editor.set_last_match(pos, pattern);
        }
        None
    } else {
        // Since no prior match exists, act as if new term search is started.
        Search::question(editor, false, false)
    }
}

struct Search {
    editor: EditorRef,
    capture: Capture,
    using_regex: bool,
    case_strict: bool,
    buf_cache: Option<String>,
    last_match: Option<(usize, Box<dyn Pattern>)>,
}

impl Search {
    fn question(editor: EditorRef, using_regex: bool, case_strict: bool) -> Option<Action> {
        let capture = editor.borrow().capture();
        let buf_cache = if using_regex {
            let buf = editor.borrow().buffer().iter().collect::<String>();
            Some(buf)
        } else {
            None
        };
        Action::as_question(
            Search {
                editor,
                capture,
                using_regex,
                case_strict,
                buf_cache,
                last_match: None,
            }
            .into_box(),
        )
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    fn restore(&mut self) {
        let mut editor = self.editor.borrow_mut();
        editor.restore(&self.capture);
        editor.render();
    }
}

impl Inquirer for Search {
    fn prompt(&self) -> String {
        format!(
            "{}search (case-{}sensitive):",
            if self.using_regex { "regex " } else { "" },
            if self.case_strict { "" } else { "in" }
        )
    }

    fn react(&mut self, _: &mut Environment, value: &str, key: &Key) -> Option<String> {
        if value.len() > 0 {
            let (pos, pattern) = match self.last_match.take() {
                Some((pos, pattern)) if *key == TAB => {
                    // Find next match using existing pattern when TAB is pressed,
                    // noting that starting position must be incremented so as not to
                    // match on same term.
                    (pos + 1, pattern)
                }
                _ => {
                    let pattern = if self.using_regex {
                        // Compile regular expression, which might fail if malformed
                        // or too large, the latter of which is unlikely in practice.
                        let regex = RegexBuilder::new(value)
                            .case_insensitive(!self.case_strict)
                            .multi_line(true)
                            .build();
                        if let Ok(regex) = regex {
                            search::using_regex(regex)
                        } else {
                            return Some(" (no match)".to_string());
                        }
                    } else {
                        search::using_term(value.to_string(), self.case_strict)
                    };
                    (self.capture.pos, pattern)
                }
            };

            // Find next match and highlight if found.
            let found = if let Some(buf) = &self.buf_cache {
                pattern.find_str(buf, pos)
            } else {
                pattern.find(&self.editor.borrow().buffer(), pos)
            };

            if let Some((start_pos, end_pos)) = found {
                let mut editor = self.editor.borrow_mut();
                editor.move_to(start_pos, Align::Center);
                editor.clear_mark();
                editor.set_soft_mark_at(end_pos);
                editor.render();
                self.last_match = Some((start_pos, pattern));
                None
            } else {
                Some(" (no match)".to_string())
            }
        } else {
            self.restore();
            self.last_match = None;
            None
        }
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some(value) if value.len() > 0 => {
                if let Some((pos, pattern)) = self.last_match.take() {
                    self.editor.borrow_mut().set_last_match(pos, pattern);
                }
            }
            _ => self.restore(),
        }
        None
    }
}

/// Operation: `open-file`
fn open_file(env: &mut Environment) -> Option<Action> {
    Open::question(derive_dir(env), None)
}

/// Operation: `open-file-top`
fn open_file_top(env: &mut Environment) -> Option<Action> {
    Open::question(derive_dir(env), Some(Placement::Top))
}

/// Operation: `open-file-bottom`
fn open_file_bottom(env: &mut Environment) -> Option<Action> {
    Open::question(derive_dir(env), Some(Placement::Bottom))
}

/// Operation: `open-file-above`
fn open_file_above(env: &mut Environment) -> Option<Action> {
    Open::question(
        derive_dir(env),
        Some(Placement::Above(env.get_active_view_id())),
    )
}

/// Operation: `open-file-below`
fn open_file_below(env: &mut Environment) -> Option<Action> {
    Open::question(
        derive_dir(env),
        Some(Placement::Below(env.get_active_view_id())),
    )
}

/// An inquirer that orchestrates the process of opening a file.
struct Open {
    /// Base directory used for joining paths entered by the user, which is typically
    /// derived from the path of the active editor.
    dir: PathBuf,

    /// Where to open the new window if specified, otherwise is replaces the editor in
    /// the current window.
    place: Option<Placement>,
}

impl Open {
    fn question(dir: PathBuf, place: Option<Placement>) -> Option<Action> {
        Action::as_question(Open { dir, place }.into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    fn open(&mut self, env: &mut Environment, path: &str) -> Option<Action> {
        let path = sys::canonicalize(self.dir.join(path)).as_string();
        let config = env.workspace.borrow().config.clone();
        match open_editor(config, &path) {
            Ok(editor) => {
                if let Some(place) = self.place {
                    if let Some((view_id, _)) = env.open_editor(editor, place, Align::Auto) {
                        env.set_active(Focus::To(view_id));
                        None
                    } else {
                        Action::echo_no_window()
                    }
                } else {
                    env.set_editor(editor, Align::Auto);
                    None
                }
            }
            Err(e) => Action::as_echo(&e),
        }
    }
}

impl Inquirer for Open {
    fn prompt(&self) -> String {
        let path = sys::pretty_path(&self.dir);
        format!("open file [{path}]:")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::file_completer(self.dir.clone())
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(path) = value {
            self.open(env, path)
        } else {
            None
        }
    }
}

/// Operation: `save-file`
fn save_file(env: &mut Environment) -> Option<Action> {
    let editor = env.get_active_editor();
    if is_file(editor) {
        match stale_editor(editor) {
            Ok(true) => SaveOverride::question(editor.clone()),
            Ok(false) => Save::save(editor),
            Err(e) => Action::as_echo(&e),
        }
    } else {
        Save::question(editor.clone())
    }
}

/// Operation: `save-file-as`
fn save_file_as(env: &mut Environment) -> Option<Action> {
    Save::question(env.get_active_editor().clone())
}

/// An inquirer that orchestrates the process of saving a file.
struct Save {
    editor: EditorRef,
}

impl Save {
    fn question(editor: EditorRef) -> Option<Action> {
        Action::as_question(Save { editor }.into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    fn save_as(editor: &EditorRef, env: &mut Environment, path: &str) -> Option<Action> {
        if is_file(editor) {
            Self::save_file(editor, path)
        } else {
            Self::save_ephemeral(editor, env, path)
        }
    }

    fn save_file(editor: &EditorRef, path: &str) -> Option<Action> {
        if let Err(e) = save_editor_as(editor, Some(path)) {
            Action::as_echo(&e)
        } else {
            Action::as_echo(&Self::echo_saved(path))
        }
    }

    fn save_ephemeral(editor: &EditorRef, env: &mut Environment, path: &str) -> Option<Action> {
        let timestamp = write_editor(editor, path);
        match timestamp {
            Ok(timestamp) => {
                // Replace ephemeral editor in current window with cloned version, keeping
                // position of cursor at same location on terminal.
                let new_editor = editor
                    .borrow()
                    .clone_as(Source::as_file(path, Some(timestamp)));
                let row = new_editor.cursor().row;
                env.set_editor(new_editor.into_ref(), Align::Row(row));

                // Reset mutable ephemeral editors, which currently only applies to
                // `@scratch`.
                if editor.borrow().is_mutable() {
                    editor.borrow_mut().reset();
                }
                Action::as_echo(&Self::echo_saved(path))
            }
            Err(e) => Action::as_echo(&e),
        }
    }

    fn save(editor: &EditorRef) -> Option<Action> {
        if let Err(e) = save_editor(editor) {
            Action::as_echo(&e)
        } else {
            let path = path_of(editor);
            Action::as_echo(&Self::echo_saved(&path.as_string()))
        }
    }

    fn echo_saved(path: &str) -> String {
        format!("{path}: saved")
    }
}

impl Inquirer for Save {
    fn prompt(&self) -> String {
        let source = source_of(&self.editor);
        format!("save {source} as:")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::file_completer(sys::working_dir())
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(path) = value {
            if Path::new(path).exists() {
                SaveExists::question(self.editor.clone(), path.to_string())
            } else {
                Self::save_as(&self.editor, env, path)
            }
        } else {
            None
        }
    }
}

/// An inquirer spawned from [`Save`] that orchestrates the saving of an editor whose
/// path provided by the user conflicts with an existing file in storage.
#[derive(Clone)]
struct SaveExists {
    editor: EditorRef,
    path: String,
}

impl SaveExists {
    fn question(editor: EditorRef, path: String) -> Option<Action> {
        Action::as_question(SaveExists { editor, path }.into_box())
    }

    fn again(&self) -> Option<Action> {
        Action::as_question(self.clone().into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }
}

impl Inquirer for SaveExists {
    fn prompt(&self) -> String {
        let path = sys::pretty_path(&self.path);
        format!("{path}: file already exists, overwrite?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => Save::save_as(&self.editor, env, &self.path),
            Some("n") => None,
            Some(_) => self.again(),
            None => None,
        }
    }
}

/// An inquirer spawned from [`Save`] that orchestrates the saving of an editor whose
/// corresponding file in storage is newer than its timestamp.
#[derive(Clone)]
struct SaveOverride {
    editor: EditorRef,
}

impl SaveOverride {
    fn question(editor: EditorRef) -> Option<Action> {
        Action::as_question(SaveOverride { editor }.into_box())
    }

    fn again(&self) -> Option<Action> {
        Action::as_question(self.clone().into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }
}

impl Inquirer for SaveOverride {
    fn prompt(&self) -> String {
        let source = source_of(&self.editor);
        format!("{source}: file in storage is newer, save anyway?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => Save::save(&self.editor),
            Some("n") => None,
            Some(_) => self.again(),
            None => None,
        }
    }
}

/// Operation: `kill-window`
fn kill_window(env: &mut Environment) -> Option<Action> {
    if env.view_map().len() > 1 {
        let editor = env.get_active_editor();
        if is_dirty_file(editor) {
            Kill::question(editor.clone(), None)
        } else {
            env.kill_window();
            None
        }
    } else if let Some((switch_id, _)) = next_unattached_editor(env) {
        let editor_id = env.get_active_editor_id();
        let editor = env.get_active_editor();
        if is_dirty_file(editor) {
            Kill::question(editor.clone(), Some((editor_id, switch_id)))
        } else {
            env.switch_editor(switch_id, Align::Auto);
            env.close_editor(editor_id);
            None
        }
    } else {
        Action::echo_cannot_close()
    }
}

/// An inquirer that orchestrates the process of killing a window with a dirty editor
/// attached.
#[derive(Clone)]
struct Kill {
    editor: EditorRef,
    close_and_switch: Option<(u32, u32)>,
}

impl Kill {
    fn question(editor: EditorRef, close_and_switch: Option<(u32, u32)>) -> Option<Action> {
        Action::as_question(
            Kill {
                editor,
                close_and_switch,
            }
            .into_box(),
        )
    }

    fn again(&self) -> Option<Action> {
        Action::as_question(self.clone().into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    fn kill(&mut self, env: &mut Environment) -> Option<Action> {
        let action = Save::save(&self.editor);
        if action.is_some() {
            self.kill_only(env);
        }
        action
    }

    fn kill_only(&mut self, env: &mut Environment) -> Option<Action> {
        if let Some((editor_id, switch_id)) = self.close_and_switch {
            env.switch_editor(switch_id, Align::Auto);
            env.close_editor(editor_id);
        } else {
            env.kill_window();
        }
        None
    }
}

impl Inquirer for Kill {
    fn prompt(&self) -> String {
        let source = source_of(&self.editor);
        format!("{source}: save?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => match stale_editor(&self.editor) {
                Ok(true) => KillOverride::question(self.editor.clone(), self.close_and_switch),
                Ok(false) => self.kill(env),
                Err(e) => Action::as_echo(&e),
            },
            Some("n") => self.kill_only(env),
            Some(_) => self.again(),
            None => None,
        }
    }
}

/// An inquirer spawned from [`Kill`] that orchestrates the saving of an editor whose
/// corresponding file in storage is newer than its timestamp.
#[derive(Clone)]
struct KillOverride {
    editor: EditorRef,
    close_and_switch: Option<(u32, u32)>,
}

impl KillOverride {
    fn question(editor: EditorRef, close_and_switch: Option<(u32, u32)>) -> Option<Action> {
        Action::as_question(
            KillOverride {
                editor,
                close_and_switch,
            }
            .into_box(),
        )
    }

    fn again(&self) -> Option<Action> {
        Action::as_question(self.clone().into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }

    fn kill(&mut self, env: &mut Environment) -> Option<Action> {
        let action = Save::save(&self.editor);
        if action.is_some() {
            if let Some((editor_id, switch_id)) = self.close_and_switch {
                env.switch_editor(switch_id, Align::Auto);
                env.close_editor(editor_id);
            } else {
                env.kill_window();
            }
        }
        action
    }
}

impl Inquirer for KillOverride {
    fn prompt(&self) -> String {
        let source = source_of(&self.editor);
        format!("{source}: file in storage is newer, save anyway?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => self.kill(env),
            Some("n") => None,
            Some(_) => self.again(),
            None => None,
        }
    }
}

/// Operation: `close-window`
fn close_window(env: &mut Environment) -> Option<Action> {
    if env.close_window().is_some() {
        None
    } else {
        Action::echo_cannot_close()
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
    let editors = unattached_editors(env, true);
    if editors.len() > 0 {
        SelectEditor::question(editors, None)
    } else {
        Action::echo_no_editors()
    }
}

/// Operation: `select-editor-top`
fn select_editor_top(env: &mut Environment) -> Option<Action> {
    let editors = unattached_editors(env, true);
    if editors.len() > 0 {
        SelectEditor::question(editors, Some(Placement::Top))
    } else {
        Action::echo_no_editors()
    }
}

/// Operation: `select-editor-bottom`
fn select_editor_bottom(env: &mut Environment) -> Option<Action> {
    let editors = unattached_editors(env, true);
    if editors.len() > 0 {
        SelectEditor::question(editors, Some(Placement::Bottom))
    } else {
        Action::echo_no_editors()
    }
}

/// Operation: `select-editor-above`
fn select_editor_above(env: &mut Environment) -> Option<Action> {
    let editors = unattached_editors(env, true);
    if editors.len() > 0 {
        SelectEditor::question(editors, Some(Placement::Above(env.get_active_view_id())))
    } else {
        Action::echo_no_editors()
    }
}

/// Operation: `select-editor-below`
fn select_editor_below(env: &mut Environment) -> Option<Action> {
    let editors = unattached_editors(env, true);
    if editors.len() > 0 {
        SelectEditor::question(editors, Some(Placement::Below(env.get_active_view_id())))
    } else {
        Action::echo_no_editors()
    }
}

/// Operation: `prev-editor`
fn prev_editor(env: &mut Environment) -> Option<Action> {
    if let Some((prev_id, _)) = prev_unattached_editor(env) {
        env.switch_editor(prev_id, Align::Auto);
    }
    None
}

/// Operation: `next-editor`
fn next_editor(env: &mut Environment) -> Option<Action> {
    if let Some((next_id, _)) = next_unattached_editor(env) {
        env.switch_editor(next_id, Align::Auto);
    }
    None
}

/// An iquirer that orchetrates the selection of an editor by name, replacing the editor
/// in the active window.
struct SelectEditor {
    /// Unattached editors available for selection.
    editors: Vec<(u32, EditorRef)>,

    /// Where to open the new window if specified, otherwise is replaces the editor in
    /// the current window.
    place: Option<Placement>,
}

impl SelectEditor {
    const PROMPT: &str = "select editor:";

    fn question(editors: Vec<(u32, EditorRef)>, place: Option<Placement>) -> Option<Action> {
        Action::as_question(SelectEditor { editors, place }.into_box())
    }

    fn into_box(self) -> Box<dyn Inquirer> {
        Box::new(self)
    }
}

impl Inquirer for SelectEditor {
    fn prompt(&self) -> String {
        Self::PROMPT.to_string()
    }

    fn completer(&self) -> Box<dyn Completer> {
        let accepted = self.editors.iter().map(|(_, e)| source_of(e)).collect();
        user::list_completer(accepted)
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(value) = value {
            let editor = self
                .editors
                .iter()
                .find(|(_, e)| source_of(e) == value)
                .map(|(id, _)| *id);
            if let Some(editor_id) = editor {
                if let Some(place) = self.place {
                    if let Some(view_id) = env.open_window(editor_id, place, Align::Auto) {
                        env.set_active(Focus::To(view_id));
                        None
                    } else {
                        Action::echo_no_window()
                    }
                } else {
                    env.switch_editor(editor_id, Align::Auto);
                    None
                }
            } else {
                Action::as_echo("{value}: editor not found")
            }
        } else {
            None
        }
    }
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
    let text = format!(
        "lines: {} | chars: {} | cursor: {}{}",
        buffer.line_of(usize::MAX) + 1,
        buffer.size(),
        c_char,
        c_code,
    );
    Action::as_echo(&text)
}

/// Operation: `tab-mode`
fn tab_mode(env: &mut Environment) -> Option<Action> {
    let mut editor = env.get_active_editor().borrow_mut();
    let hard = editor.get_tab();
    editor.set_tab(!hard);
    if hard {
        Action::as_echo("soft tabs enabled")
    } else {
        Action::as_echo("hard tabs enabled")
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
    let editor = Editor::mutable(config, Source::as_file(path, time), Some(buffer));
    Ok(editor.into_ref())
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
        .unwrap_or_else(|| path_of(editor).as_string());
    write_editor(editor, &path).map(|time| update_editor(editor, &path, time))
}

/// Writes the buffer of `editor` to `path` and returns the resulting file modification
/// time.
fn write_editor(editor: &EditorRef, path: &str) -> Result<SystemTime> {
    let _ = io::write_file(path, &editor.borrow().buffer())?;
    io::get_time(path)
}

/// Clears the dirty flag on `editor` and sets its source as _file_ using `path` and
/// its modification `timestamp`.
fn update_editor(editor: &EditorRef, path: &str, timestamp: SystemTime) {
    let mut editor = editor.borrow_mut();
    editor.assume(Source::as_file(path, Some(timestamp)));
    editor.clear_dirty();
}

/// Returns `true` if `editor` has a modification time older than the modification time
/// of the file in storage.
fn stale_editor(editor: &EditorRef) -> Result<bool> {
    let editor = editor.borrow();
    let stale = if let Source::File(path, Some(timestamp)) = editor.source() {
        io::get_time(path)? > *timestamp
    } else {
        false
    };
    Ok(stale)
}

/// Returns an ordered collection of _dirty_ editors.
fn dirty_editors(env: &Environment) -> Vec<EditorRef> {
    env.editor_map()
        .iter()
        .filter(|(_, e)| is_dirty_file(e))
        .map(|(_, e)| e.clone())
        .collect()
}

/// Returns an ordered collection of editor ids and editors for those editors that are
/// not attached to a window.
///
/// When `ephemerals` is `true`, the collection also contains ephemeral editors,
/// otherwise they are filtered out.
fn unattached_editors(env: &Environment, ephemerals: bool) -> Vec<(u32, EditorRef)> {
    let attached = env.view_map().values().cloned().collect::<Vec<_>>();
    env.editor_map()
        .iter()
        .filter(|(id, e)| !attached.contains(id) && (is_file(e) || is_ephemeral(e) == ephemerals))
        .map(|(id, e)| (*id, e.clone()))
        .collect()
}

/// Returns the editor id and editor of the previous unattached editor relative to the
/// editor in the current window, or `None` if all editors are attached.
fn prev_unattached_editor(env: &Environment) -> Option<(u32, EditorRef)> {
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
fn next_unattached_editor(env: &Environment) -> Option<(u32, EditorRef)> {
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

/// Returns the path associated with `editor`.
fn path_of(editor: &EditorRef) -> PathBuf {
    if let Source::File(path, _) = editor.borrow().source() {
        PathBuf::from(path)
    } else {
        PathBuf::from("")
    }
}

/// Returns the source associated with `editor`.
fn source_of(editor: &EditorRef) -> String {
    editor.borrow().source().to_string()
}

/// Returns `true` if source of `editor` is a _file_.
fn is_file(editor: &EditorRef) -> bool {
    editor.borrow().source().is_file()
}

/// Returns `true` if source of `editor` is an _ephemeral_.
fn is_ephemeral(editor: &EditorRef) -> bool {
    editor.borrow().source().is_ephemeral()
}

/// Returns `true` if source of `editor` is a _file_ and is dirty.
fn is_dirty_file(editor: &EditorRef) -> bool {
    let editor = editor.borrow();
    editor.source().is_file() && editor.is_dirty()
}

/// Returns the base directory of the active editor.
fn derive_dir(env: &mut Environment) -> PathBuf {
    derive_dir_from(env.get_active_editor())
}

/// Returns the base directory derived from `editor`, which is canonicalized so long
/// as no failures occur along the way, otherwise it resorts to a directory path of
/// `"."`
fn derive_dir_from(editor: &EditorRef) -> PathBuf {
    base_dir(editor)
        .canonicalize()
        .unwrap_or_else(|_| sys::this_dir())
}

/// Returns the base directory of the path associated with `editor` so long as the
/// editor source is a _file_, otherwise the current working directory is assumed.
///
/// `None` is returned if the base directory cannot be determined, possibly from a
/// failure to get the current working directory.
fn base_dir(editor: &EditorRef) -> PathBuf {
    if let Source::File(path, _) = editor.borrow().source() {
        sys::base_dir(path)
    } else {
        sys::working_dir()
    }
}

/// Predefined mapping of editing operations to editing functions.
#[rustfmt::skip]
pub const OP_MAPPINGS: [(&str, OpFn, &str); 82] = [
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
];

/// Returns a mapping of editing operations to editing functions.
pub fn init_op_map() -> OpMap {
    let mut op_map = OpMap::new();
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
