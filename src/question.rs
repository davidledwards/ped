//! A collection of questions used by editing operations.

use crate::config::ConfigurationRef;
use crate::ed;
use crate::editor::{Align, Capture, EditorRef};
use crate::env::{Environment, Focus};
use crate::key::{Key, SHIFT_TAB, TAB};
use crate::operation::Action;
use crate::search::{self, Match, Pattern};
use crate::source::Source;
use crate::sys;
use crate::sys::AsString;
use crate::user::{self, Completer, Question};
use crate::workspace::Placement;
use regex_lite::RegexBuilder;
use std::path::{Path, PathBuf};

/// Returns a question that orchestrates the _quit_ process, which may involve saving dirty
/// editors derived from `env`.
pub fn quit(env: &Environment) -> Option<Action> {
    let dirty = ed::dirty_editors(env);
    if dirty.len() > 0 {
        Action::question(Quit::new(dirty).into())
    } else {
        Action::quit()
    }
}

fn quit_continue(dirty: &[EditorRef]) -> Option<Action> {
    if dirty.len() > 1 {
        let dirty = dirty[1..].to_vec();
        Action::question(Quit::new(dirty).into())
    } else {
        Action::quit()
    }
}

fn quit_override(dirty: Vec<EditorRef>) -> Option<Action> {
    Action::question(QuitOverride::new(dirty).into())
}

/// Returns a question that orchestrates moving the cursor in `editor` to a specific line.
pub fn goto_line(editor: EditorRef) -> Option<Action> {
    Action::question(GotoLine::new(editor).into())
}

/// Returns a question that inserts a Unicode character into `editor`, where `radix`
/// defines the base of the user-provided code point.
pub fn insert_unicode(editor: EditorRef, radix: u32) -> Option<Action> {
    Action::question(InsertUnicode::new(editor, radix).into())
}

/// Returns a question that incrementally searches `editor` for matches of the user-provided
/// term, and where `case_strict` determines case-sensitivity.
pub fn search_term(editor: EditorRef, case_strict: bool) -> Option<Action> {
    Action::question(Search::new(editor, false, case_strict).into())
}

/// Returns a question that incrementally searches `editor` for matches of the user-provided
/// regular expression, and where `case_strict` determines case-sensitivity.
pub fn search_regex(editor: EditorRef, case_strict: bool) -> Option<Action> {
    Action::question(Search::new(editor, true, case_strict).into())
}

/// Returns a question that orchestrates the process of opening a file relative to `dir`
/// and whose optional `place` determines the relative placement of a new window.
pub fn open(dir: PathBuf, place: Option<Placement>) -> Option<Action> {
    Action::question(Open::new(dir, place).into())
}

/// Returns a question that orchestrates the process of saving `editor`.
pub fn save(editor: EditorRef) -> Option<Action> {
    Action::question(Save::new(editor).into())
}

fn save_exists(editor: EditorRef, path: String) -> Option<Action> {
    Action::question(SaveExists::new(editor, path).into())
}

/// Returns a question that orchestrates the saving of `editor` whose corresponding file in
/// storage is newer than its timestamp.
pub fn save_override(editor: EditorRef) -> Option<Action> {
    Action::question(SaveOverride::new(editor).into())
}

/// Saves `editor` and returns a corresponding _echo_ action.
pub fn save_now(editor: &EditorRef) -> Option<Action> {
    Save::save(editor)
}

/// Returns a question that orchestrates the killing of `editor` and closing its window
/// or optionally switching to another editor.
pub fn kill(editor: EditorRef, close_and_switch: Option<(u32, u32)>) -> Option<Action> {
    Action::question(Kill::new(editor, close_and_switch).into())
}

fn kill_override(editor: EditorRef, close_and_switch: Option<(u32, u32)>) -> Option<Action> {
    Action::question(KillOverride::new(editor, close_and_switch).into())
}

/// Returns a question that orchetrates the selection of an editor in `editors` by name,
/// optionally opening a new window whose placement is defined by `place`.
pub fn select(editors: Vec<(u32, EditorRef)>, place: Option<Placement>) -> Option<Action> {
    Action::question(Select::new(editors, place).into())
}

/// Returns a question that orchestrates the execution of any editing operation, all of
/// which are discovered via `env`.
pub fn run(env: &Environment) -> Option<Action> {
    let config = env.workspace.borrow().config.clone();
    Action::question(Run::new(config).into())
}

impl<T: Question + 'static> From<T> for Box<dyn Question> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

fn repeat_question<T: Question + Clone + 'static>(question: &mut T) -> Option<Action> {
    Action::question(question.clone().into())
}

struct Quit {
    /// List of dirty editors.
    dirty: Vec<EditorRef>,
}

impl Quit {
    /// Starts the process of saving dirty editors before quitting.
    fn new(dirty: Vec<EditorRef>) -> Quit {
        Quit { dirty }
    }

    /// Saves the first dirty editor and then continues to the next editor.
    fn save_first(&mut self) -> Option<Action> {
        let editor = &self.dirty[0];
        match ed::stale_editor(editor) {
            Ok(true) => quit_override(self.dirty.clone()),
            Ok(false) => {
                if let Err(e) = ed::save_editor(editor) {
                    Action::echo(&e)
                } else {
                    quit_continue(&self.dirty)
                }
            }
            Err(e) => Action::echo(&e),
        }
    }

    /// Saves all dirty editors.
    fn save_all(&mut self) -> Option<Action> {
        let mut dirty_iter = self.dirty.iter();
        while let Some(editor) = dirty_iter.next() {
            match ed::stale_editor(editor) {
                Ok(true) => {
                    let mut dirty = vec![editor.clone()];
                    dirty.extend(dirty_iter.cloned());
                    return quit_override(dirty);
                }
                Ok(false) => {
                    if let Err(e) = ed::save_editor(editor) {
                        return Action::echo(&e);
                    }
                }
                Err(e) => {
                    return Action::echo(&e);
                }
            }
        }
        Action::quit()
    }
}

impl Question for Quit {
    fn prompt(&self) -> String {
        let source = ed::source_of(&self.dirty[0]);
        format!("{source}: save?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_all_completer()
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => self.save_first(),
            Some("a") => self.save_all(),
            Some("n") => quit_continue(&self.dirty),
            Some(_) => quit(env),
            None => None,
        }
    }
}

#[derive(Clone)]
struct QuitOverride {
    /// List of dirty editors, where the first entry is pertinent to this flow.
    dirty: Vec<EditorRef>,
}

impl QuitOverride {
    fn new(dirty: Vec<EditorRef>) -> QuitOverride {
        QuitOverride { dirty }
    }

    fn save(&mut self) -> Option<Action> {
        if let Err(e) = ed::save_editor(&self.dirty[0]) {
            Action::echo(&e)
        } else {
            quit_continue(&self.dirty)
        }
    }
}

impl Question for QuitOverride {
    fn prompt(&self) -> String {
        let source = ed::source_of(&self.dirty[0]);
        format!("{source}: file in storage is newer, save anyway?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => self.save(),
            Some("n") => quit_continue(&self.dirty),
            Some(_) => repeat_question(self),
            None => None,
        }
    }
}

struct GotoLine {
    /// Editor in context.
    editor: EditorRef,

    /// Current state of `editor` prior to operation.
    capture: Capture,
}

impl GotoLine {
    fn new(editor: EditorRef) -> GotoLine {
        let capture = editor.borrow().capture();
        GotoLine { editor, capture }
    }

    fn restore(&mut self) {
        let mut editor = self.editor.borrow_mut();
        editor.restore(&self.capture);
        editor.render();
    }
}

impl Question for GotoLine {
    fn prompt(&self) -> String {
        "goto line[,col]:".to_string()
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::line_column_completer()
    }

    fn react(&mut self, _: &mut Environment, value: &str, _: &Key) -> Option<String> {
        let value = value.trim();
        if value.len() > 0 {
            match user::line_column_parse(value) {
                Some((line, col)) => {
                    let line = line.saturating_sub(1);
                    let col = col.unwrap_or(0).saturating_sub(1);
                    let mut editor = self.editor.borrow_mut();
                    editor.move_line_col(line, col, Align::Center);
                    editor.render();
                    None
                }
                None => Some(" (invalid)".to_string()),
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

struct InsertUnicode {
    /// Editor in context.
    editor: EditorRef,

    /// Only values of `10` and `16` are supported.
    radix: u32,
}

impl InsertUnicode {
    fn new(editor: EditorRef, radix: u32) -> InsertUnicode {
        debug_assert!(radix == 10 || radix == 16);
        InsertUnicode { editor, radix }
    }

    fn parse(&self, value: &str) -> Option<char> {
        user::number_parse(value, self.radix).and_then(char::from_u32)
    }
}

impl Question for InsertUnicode {
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
            if let Some(c) = self.parse(value) {
                if c.is_control() {
                    None
                } else {
                    Some(format!(" '{c}'"))
                }
            } else {
                Some(" (invalid)".to_string())
            }
        } else {
            None
        }
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(value) = value
            && let Some(c) = self.parse(value)
        {
            let mut editor = self.editor.borrow_mut();
            if editor.is_mutable() {
                editor.clear_mark();
                editor.insert_char(c);
                editor.render();
                None
            } else {
                Action::echo("editor is readonly")
            }
        } else {
            None
        }
    }
}

struct Search {
    /// Editor in context.
    editor: EditorRef,

    /// Current state of `editor` prior to search.
    capture: Capture,

    /// Search using regex if `true`, otherwise using term.
    using_regex: bool,

    /// Apply strict case matching if `true`.
    case_strict: bool,

    /// Copy of editor buffer when searching using regex, which is an unfortunate
    /// limitation of current regex library.
    buf_cache: Option<String>,

    /// List of pairs containing starting and ending buffer positions of matches
    /// pertaining to `last_value`.
    ///
    /// Note that matches build incrementally, so list is not necessarily exhaustive of
    /// all possible matches.
    ///
    /// It must hold true that all start positions in this vector are strictly
    /// monotonically increasing. This property is intuitive because subsequent matches
    /// are always further along in buffer.
    match_cache: Vec<(usize, usize)>,

    /// A pair containing an index into `match_cache` representing the current match
    /// position and the applicable pattern, or `None` if nothing matches `pattern`.
    match_index: Option<(usize, Box<dyn Pattern>)>,

    /// Most recently seen value of input.
    last_value: String,
}

impl Search {
    fn new(editor: EditorRef, using_regex: bool, case_strict: bool) -> Search {
        let capture = editor.borrow().capture();
        let buf_cache = if using_regex {
            let buf = editor.borrow().buffer().iter().collect::<String>();
            Some(buf)
        } else {
            None
        };

        // Use selected text if present to initialize search term, though ignore if
        // regular expression being used.
        let last_value = if using_regex {
            String::new()
        } else {
            // Ignore selected text if any of its content contains control characters.
            capture
                .mark
                .map(|mark| editor.borrow().copy_mark(mark))
                .and_then(|text| {
                    if text.iter().any(|c| c.is_control()) {
                        None
                    } else {
                        Some(text.iter().collect())
                    }
                })
                .unwrap_or_default()
        };

        // Prime search such that pressing TAB will find next match.
        let (match_cache, match_index) = if last_value.len() > 0 {
            let start_pos = capture.pos;
            let end_pos = start_pos + last_value.len();
            let pattern = search::using_term(&last_value, case_strict);
            (vec![(start_pos, end_pos)], Some((0, pattern)))
        } else {
            (vec![], None)
        };

        Search {
            editor,
            capture,
            using_regex,
            case_strict,
            buf_cache,
            match_cache,
            match_index,
            last_value,
        }
    }

    fn find_first(&mut self, value: &str) -> Option<String> {
        self.match_cache.clear();
        let pattern = if self.using_regex {
            // Compile regular expression, which might fail if malformed
            // or too large, the latter of which is unlikely in practice.
            RegexBuilder::new(value)
                .case_insensitive(!self.case_strict)
                .multi_line(true)
                .build()
                .map(search::using_regex)
                .ok()
        } else {
            Some(search::using_term(value, self.case_strict))
        };
        self.match_index = if let Some(pattern) = pattern
            && let Some(Match(start_pos, end_pos)) = self.find_match(&*pattern, self.capture.pos)
        {
            self.highlight_match(start_pos, end_pos);
            self.match_cache.push((start_pos, end_pos));
            Some((0, pattern))
        } else {
            None
        };
        self.match_hint()
    }

    fn find_next(&mut self) -> Option<String> {
        self.match_index = match self.match_index.take() {
            Some((index, pattern)) if index == self.match_cache.len() - 1 => {
                // Find next match position since current index at end of cache.
                let pos = self.match_cache[index].0 + 1;
                if let Some(Match(start_pos, end_pos)) = self.find_match(&*pattern, pos) {
                    self.highlight_match(start_pos, end_pos);
                    if start_pos == self.match_cache[0].0 {
                        // Next match essentially wrapped.
                        Some((0, pattern))
                    } else {
                        // Add next match to cache.
                        self.match_cache.push((start_pos, end_pos));
                        Some((index + 1, pattern))
                    }
                } else {
                    None
                }
            }
            Some((index, pattern)) => {
                // Next match position already cached.
                let (start_pos, end_pos) = self.match_cache[index + 1];
                self.highlight_match(start_pos, end_pos);
                Some((index + 1, pattern))
            }
            None => None,
        };
        self.match_hint()
    }

    fn find_prev(&mut self) -> Option<String> {
        self.match_index = match self.match_index.take() {
            Some((index, pattern)) => {
                let index = index.saturating_sub(1);
                let (start_pos, end_pos) = self.match_cache[index];
                self.highlight_match(start_pos, end_pos);
                Some((index, pattern))
            }
            None => None,
        };
        self.match_hint()
    }

    fn find_match(&self, pattern: &dyn Pattern, pos: usize) -> Option<Match> {
        if let Some(buf) = &self.buf_cache {
            pattern.find_str(buf, pos)
        } else {
            pattern.find(&self.editor.borrow().buffer(), pos)
        }
    }

    fn highlight_match(&mut self, start_pos: usize, end_pos: usize) {
        let mut editor = self.editor.borrow_mut();
        editor.move_to(start_pos, Align::Center);
        editor.clear_mark();
        editor.set_soft_mark_at(end_pos);
        editor.render();
    }

    fn match_hint(&self) -> Option<String> {
        match self.match_index {
            Some(_) => None,
            None => Some(" (no match)".to_string()),
        }
    }

    fn restore(&mut self) {
        let mut editor = self.editor.borrow_mut();
        editor.restore(&self.capture);
        editor.render();
    }
}

impl Question for Search {
    fn prompt(&self) -> String {
        format!(
            "{}search (case-{}sensitive):",
            if self.using_regex { "regex " } else { "" },
            if self.case_strict { "" } else { "in" }
        )
    }

    fn value(&self) -> Option<String> {
        if self.last_value.len() == 0 {
            None
        } else {
            Some(self.last_value.clone())
        }
    }

    fn react(&mut self, _: &mut Environment, value: &str, key: &Key) -> Option<String> {
        if *key == TAB {
            self.find_next()
        } else if *key == SHIFT_TAB {
            self.find_prev()
        } else if value == self.last_value {
            None
        } else {
            let hint = if value.len() > 0 {
                self.find_first(value)
            } else {
                self.restore();
                None
            };
            self.last_value = value.to_string();
            hint
        }
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(value) = value
            && value.len() > 0
            && let Some((index, pattern)) = self.match_index.take()
        {
            self.editor
                .borrow_mut()
                .set_last_match(self.match_cache[index].0, pattern);
        } else {
            self.restore();
        }
        None
    }
}

struct Open {
    /// Base directory used for joining paths entered by the user, which is typically
    /// derived from the path of the active editor.
    dir: PathBuf,

    /// Where to open the new window if specified, otherwise is replaces the editor in
    /// the current window.
    place: Option<Placement>,
}

impl Open {
    fn new(dir: PathBuf, place: Option<Placement>) -> Open {
        Open { dir, place }
    }

    fn open(&mut self, env: &mut Environment, path: &str) -> Option<Action> {
        let path = sys::canonicalize(self.dir.join(path)).as_string();
        let config = env.workspace.borrow().config.clone();
        match ed::open_editor(config, &path) {
            Ok(editor) => {
                if let Some(place) = self.place {
                    if let Some((view_id, _)) = env.open_editor(editor, place, Align::Auto) {
                        env.set_active(Focus::To(view_id));
                        None
                    } else {
                        Action::echo("unable to create new window")
                    }
                } else {
                    env.set_editor(editor, Align::Auto);
                    None
                }
            }
            Err(e) => Action::echo(&e),
        }
    }
}

impl Question for Open {
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

struct Save {
    /// Editor in context.
    editor: EditorRef,
}

impl Save {
    fn new(editor: EditorRef) -> Save {
        Save { editor }
    }

    fn save_as(editor: &EditorRef, env: &mut Environment, path: &str) -> Option<Action> {
        if ed::is_file(editor) {
            Self::save_file(editor, path)
        } else {
            Self::save_ephemeral(editor, env, path)
        }
    }

    fn save_file(editor: &EditorRef, path: &str) -> Option<Action> {
        if let Err(e) = ed::save_editor_as(editor, Some(path)) {
            Action::echo(&e)
        } else {
            Action::echo(&Self::echo_saved(path))
        }
    }

    fn save_ephemeral(editor: &EditorRef, env: &mut Environment, path: &str) -> Option<Action> {
        let timestamp = ed::write_editor(editor, path);
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
                Action::echo(&Self::echo_saved(path))
            }
            Err(e) => Action::echo(&e),
        }
    }

    fn save(editor: &EditorRef) -> Option<Action> {
        if let Err(e) = ed::save_editor(editor) {
            Action::echo(&e)
        } else {
            let path = ed::path_of(editor);
            Action::echo(&Self::echo_saved(&path.as_string()))
        }
    }

    fn echo_saved(path: &str) -> String {
        format!("{path}: saved")
    }
}

impl Question for Save {
    fn prompt(&self) -> String {
        let source = ed::source_of(&self.editor);
        format!("save {source} as:")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::file_completer(sys::working_dir())
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(path) = value {
            if Path::new(path).exists() {
                save_exists(self.editor.clone(), path.to_string())
            } else {
                Self::save_as(&self.editor, env, path)
            }
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct SaveExists {
    /// Editor in context.
    editor: EditorRef,

    /// Path of file that already exists.
    path: String,
}

impl SaveExists {
    fn new(editor: EditorRef, path: String) -> SaveExists {
        SaveExists { editor, path }
    }
}

impl Question for SaveExists {
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
            Some(_) => repeat_question(self),
            None => None,
        }
    }
}

#[derive(Clone)]
struct SaveOverride {
    /// Editor in context.
    editor: EditorRef,
}

impl SaveOverride {
    fn new(editor: EditorRef) -> SaveOverride {
        SaveOverride { editor }
    }
}

impl Question for SaveOverride {
    fn prompt(&self) -> String {
        let source = ed::source_of(&self.editor);
        format!("{source}: file in storage is newer, save anyway?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => Save::save(&self.editor),
            Some("n") => None,
            Some(_) => repeat_question(self),
            None => None,
        }
    }
}

#[derive(Clone)]
struct Kill {
    /// Editor in context.
    editor: EditorRef,
    close_and_switch: Option<(u32, u32)>,
}

impl Kill {
    fn new(editor: EditorRef, close_and_switch: Option<(u32, u32)>) -> Kill {
        Kill {
            editor,
            close_and_switch,
        }
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

impl Question for Kill {
    fn prompt(&self) -> String {
        let source = ed::source_of(&self.editor);
        format!("{source}: save?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => match ed::stale_editor(&self.editor) {
                Ok(true) => kill_override(self.editor.clone(), self.close_and_switch),
                Ok(false) => self.kill(env),
                Err(e) => Action::echo(&e),
            },
            Some("n") => self.kill_only(env),
            Some(_) => repeat_question(self),
            None => None,
        }
    }
}

#[derive(Clone)]
struct KillOverride {
    /// Editor in context.
    editor: EditorRef,
    close_and_switch: Option<(u32, u32)>,
}

impl KillOverride {
    fn new(editor: EditorRef, close_and_switch: Option<(u32, u32)>) -> KillOverride {
        KillOverride {
            editor,
            close_and_switch,
        }
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

impl Question for KillOverride {
    fn prompt(&self) -> String {
        let source = ed::source_of(&self.editor);
        format!("{source}: file in storage is newer, save anyway?")
    }

    fn completer(&self) -> Box<dyn Completer> {
        user::yes_no_completer()
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        match value {
            Some("y") => self.kill(env),
            Some("n") => None,
            Some(_) => repeat_question(self),
            None => None,
        }
    }
}

struct Select {
    /// Unattached editors available for selection.
    editors: Vec<(u32, EditorRef)>,

    /// Where to open the new window if specified, otherwise is replaces the editor in
    /// the current window.
    place: Option<Placement>,
}

impl Select {
    fn new(editors: Vec<(u32, EditorRef)>, place: Option<Placement>) -> Select {
        Select { editors, place }
    }
}

impl Question for Select {
    fn prompt(&self) -> String {
        const PROMPT: &str = "select editor:";
        PROMPT.to_string()
    }

    fn completer(&self) -> Box<dyn Completer> {
        let accepted = self.editors.iter().map(|(_, e)| ed::source_of(e)).collect();
        user::list_completer(accepted)
    }

    fn respond(&mut self, env: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(value) = value {
            let editor = self
                .editors
                .iter()
                .find(|(_, e)| ed::source_of(e) == value)
                .map(|(id, _)| *id);
            if let Some(editor_id) = editor {
                if let Some(place) = self.place {
                    if let Some(view_id) = env.open_window(editor_id, place, Align::Auto) {
                        env.set_active(Focus::To(view_id));
                        None
                    } else {
                        Action::echo("unable to create new window")
                    }
                } else {
                    env.switch_editor(editor_id, Align::Auto);
                    None
                }
            } else {
                Action::echo(&format!("{value}: editor not found"))
            }
        } else {
            None
        }
    }
}

struct Run {
    /// Available operations derived from `config`.
    config: ConfigurationRef,
}

impl Run {
    fn new(config: ConfigurationRef) -> Run {
        Run { config }
    }
}

impl Question for Run {
    fn prompt(&self) -> String {
        "run:".to_string()
    }

    fn completer(&self) -> Box<dyn Completer> {
        let mut accepted = self
            .config
            .bindings
            .ops()
            .keys()
            .map(|op| op.to_string())
            .collect::<Vec<_>>();
        accepted.sort();
        user::list_completer(accepted)
    }

    fn respond(&mut self, _: &mut Environment, value: Option<&str>) -> Option<Action> {
        if let Some(value) = value {
            Action::run(value)
        } else {
            None
        }
    }
}
