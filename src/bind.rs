//! Key bindings.

use crate::editor::{Editor, Focus};
use crate::error::Result;
use crate::key::{Ctrl, Key, Shift};
use std::collections::{HashMap, HashSet};

/// A function pointer that implements an editing operation.
pub type Binding = fn(&mut Editor, &Key) -> Result<()>;

/// Map of canonical editing operations to editing functions.
type EditMap = HashMap<&'static str, Binding>;

/// Map of keys to canonical names.
type KeyMap = HashMap<Key, &'static str>;

/// Map of canonical key names to canonical editing operations.
type BindingMap = HashMap<&'static str, &'static str>;

/// A binding of keys to editing operations.
///
/// Bindings are essentially a mapping of canonical key names to canonical editing
/// operations. Such associations are made at runtime, essentially allowing custom
/// bindings.
pub struct Bindings {
    key_map: KeyMap,
    edit_map: EditMap,
    binding_map: BindingMap,
}

impl Bindings {
    pub fn with_bindings(bindings: &[(String, String)]) -> Result<Bindings> {
        let mut this = Bindings {
            key_map: init_key_map(),
            edit_map: init_edit_map(),
            binding_map: BindingMap::new(),
        };
        this.bind(bindings)?;
        Ok(this)
    }

    pub fn lookup(&self, key: &Key) -> Option<&Binding> {
        self.map_key(key)
            .and_then(|name| self.binding_map.get(name))
            .and_then(|op| self.edit_map.get(op as &str))
    }

    fn map_key(&self, key: &Key) -> Option<&'static str> {
        match key {
            Key::Char(_) => Some("char"),
            _ => self.key_map.get(key).map(|name| *name),
        }
    }

    fn bind(&mut self, bindings: &[(String, String)]) -> Result<()> {
        // Extract canonical key names so provided bindings can be verified to exist
        // before actually trying to bind.
        //
        // A special "char" name is added since this needs to be resolved, but cannot
        // exist in predefined key mappings because all characters map to this name.
        let mut key_names: HashSet<&'static str> = self.key_map.values().cloned().collect();
        key_names.insert("char");

        for (name, op) in bindings {
            if let Some(name) = key_names.get(name.as_str()) {
                if let Some((op, _)) = self.edit_map.get_key_value(op.as_str()) {
                    self.binding_map.insert(name, op);
                } else {
                    return Err(
                        format!("key binding ({name} -> {op}): {op}: unknown operation").into(),
                    );
                }
            } else {
                return Err(format!("key binding ({name} -> {op}): {name}: unknown key").into());
            }
        }
        Ok(())
    }
}

impl Default for Bindings {
    fn default() -> Bindings {
        let bindings: Vec<(String, String)> = DEFAULT_BINDINGS
            .iter()
            .map(|(name, op)| (name.to_string(), op.to_string()))
            .collect();
        match Bindings::with_bindings(&bindings) {
            Ok(bindings) => bindings,
            Err(e) => panic!("{e:?}"),
        }
    }
}

/// Default key bindings that associate canonical key names to canonical editing operations.
const DEFAULT_BINDINGS: [(&'static str, &'static str); 23] = [
    ("char", "insert-char"),
    ("delete", "delete-char-left"),
    ("ctrl-h", "delete-char-left"),
    ("ctrl-d", "delete-char-right"),
    ("up", "move-up"),
    ("ctrl-p", "move-up"),
    ("down", "move-down"),
    ("ctrl-n", "move-down"),
    ("left", "move-left"),
    ("ctrl-b", "move-left"),
    ("right", "move-right"),
    ("ctrl-f", "move-right"),
    ("page-up", "move-page-up"),
    ("page-down", "move-page-down"),
    ("ctrl-home", "move-top"),
    ("ctrl-end", "move-bottom"),
    ("shift-ctrl-up", "scroll-up"),
    ("shift-ctrl-down", "scroll-down"),
    ("home", "move-begin-line"),
    ("ctrl-a", "move-begin-line"),
    ("end", "move-end-line"),
    ("ctrl-e", "move-end-line"),
    ("ctrl-l", "redraw"),
];

/// Predefined key mappings that associate well known [`Key`]s with canonical names.
///
/// Canonical names are used for the runtime binding of keys to editing operations,
/// which themselves are named and well known.
///
/// Note that [`Key::Char`] is absent from these mappings because of the impracticality
/// of mapping all possible characters.
const KEY_MAPPINGS: [(Key, &'static str); 87] = [
    (Key::Control(0), "ctrl-@"),
    (Key::Control(1), "ctrl-a"),
    (Key::Control(2), "ctrl-b"),
    (Key::Control(3), "ctrl-c"),
    (Key::Control(4), "ctrl-d"),
    (Key::Control(5), "ctrl-e"),
    (Key::Control(6), "ctrl-f"),
    (Key::Control(7), "ctrl-g"),
    (Key::Control(8), "ctrl-h"),
    (Key::Control(9), "ctrl-i"),
    (Key::Control(10), "ctrl-j"),
    (Key::Control(11), "ctrl-k"),
    (Key::Control(12), "ctrl-l"),
    (Key::Control(13), "ctrl-m"),
    (Key::Control(14), "ctrl-n"),
    (Key::Control(15), "ctrl-o"),
    (Key::Control(16), "ctrl-p"),
    (Key::Control(17), "ctrl-q"),
    (Key::Control(18), "ctrl-r"),
    (Key::Control(19), "ctrl-s"),
    (Key::Control(20), "ctrl-t"),
    (Key::Control(21), "ctrl-u"),
    (Key::Control(22), "ctrl-v"),
    (Key::Control(23), "ctrl-w"),
    (Key::Control(24), "ctrl-x"),
    (Key::Control(25), "ctrl-y"),
    (Key::Control(26), "ctrl-z"),
    (Key::Control(27), "ctrl-["),
    (Key::Control(28), "ctrl-\\"),
    (Key::Control(29), "ctrl-]"),
    (Key::Control(30), "ctrl-^"),
    (Key::Control(31), "ctrl-_"),
    (Key::Delete, "delete"),
    (Key::Insert, "insert"),
    (Key::ShiftTab, "shift-tab"),
    (Key::Up(Shift::Off, Ctrl::Off), "up"),
    (Key::Up(Shift::On, Ctrl::Off), "shift-up"),
    (Key::Up(Shift::Off, Ctrl::On), "ctrl-up"),
    (Key::Up(Shift::On, Ctrl::On), "shift-ctrl-up"),
    (Key::Down(Shift::Off, Ctrl::Off), "down"),
    (Key::Down(Shift::On, Ctrl::Off), "shift-down"),
    (Key::Down(Shift::Off, Ctrl::On), "ctrl-down"),
    (Key::Down(Shift::On, Ctrl::On), "shift-ctrl-down"),
    (Key::Left(Shift::Off, Ctrl::Off), "left"),
    (Key::Left(Shift::On, Ctrl::Off), "shift-left"),
    (Key::Left(Shift::Off, Ctrl::On), "ctrl-left"),
    (Key::Left(Shift::On, Ctrl::On), "shift-ctrl-left"),
    (Key::Right(Shift::Off, Ctrl::Off), "right"),
    (Key::Right(Shift::On, Ctrl::Off), "shift-right"),
    (Key::Right(Shift::Off, Ctrl::On), "ctrl-right"),
    (Key::Right(Shift::On, Ctrl::On), "shift-ctrl-right"),
    (Key::Home(Shift::Off, Ctrl::Off), "home"),
    (Key::Home(Shift::On, Ctrl::Off), "shift-home"),
    (Key::Home(Shift::Off, Ctrl::On), "ctrl-home"),
    (Key::Home(Shift::On, Ctrl::On), "shift-ctrl-home"),
    (Key::End(Shift::Off, Ctrl::Off), "end"),
    (Key::End(Shift::On, Ctrl::Off), "shift-end"),
    (Key::End(Shift::Off, Ctrl::On), "ctrl-end"),
    (Key::End(Shift::On, Ctrl::On), "shift-ctrl-end"),
    (Key::PageUp(Shift::Off, Ctrl::Off), "page-up"),
    (Key::PageUp(Shift::On, Ctrl::Off), "shift-page-up"),
    (Key::PageUp(Shift::Off, Ctrl::On), "ctrl-page-up"),
    (Key::PageUp(Shift::On, Ctrl::On), "shift-ctrl-page-up"),
    (Key::PageDown(Shift::Off, Ctrl::Off), "page-down"),
    (Key::PageDown(Shift::On, Ctrl::Off), "shift-page-down"),
    (Key::PageDown(Shift::Off, Ctrl::On), "ctrl-page-down"),
    (Key::PageDown(Shift::On, Ctrl::On), "shift-ctrl-page-down"),
    (Key::Function(1), "fn-1"),
    (Key::Function(2), "fn-2"),
    (Key::Function(3), "fn-3"),
    (Key::Function(4), "fn-4"),
    (Key::Function(5), "fn-5"),
    (Key::Function(6), "fn-6"),
    (Key::Function(7), "fn-7"),
    (Key::Function(8), "fn-8"),
    (Key::Function(9), "fn-9"),
    (Key::Function(10), "fn-10"),
    (Key::Function(11), "fn-11"),
    (Key::Function(12), "fn-12"),
    (Key::Function(13), "fn-13"),
    (Key::Function(14), "fn-14"),
    (Key::Function(15), "fn-15"),
    (Key::Function(16), "fn-16"),
    (Key::Function(17), "fn-17"),
    (Key::Function(18), "fn-18"),
    (Key::Function(19), "fn-19"),
    (Key::Function(20), "fn-20"),
];

fn init_key_map() -> KeyMap {
    let mut key_map = KeyMap::new();
    for (key, name) in KEY_MAPPINGS {
        key_map.insert(key, name);
    }
    key_map
}

/// Predefined edit mappings that associate the canonical names of well known editing
/// operations with function pointers that carry out those operations.
///
/// Canonical names are used for the runtime binding of keys to editing operations,
/// which themselves are named and well known.
const EDIT_MAPPINGS: [(&'static str, Binding); 16] = [
    ("insert-char", do_insert_char),
    ("delete-char-left", do_delete_char_left),
    ("delete-char-right", do_delete_char_right),
    ("move-up", do_move_up),
    ("move-down", do_move_down),
    ("move-left", do_move_left),
    ("move-right", do_move_right),
    ("move-page-up", do_move_page_up),
    ("move-page-down", do_move_page_down),
    ("move-top", do_move_top),
    ("move-bottom", do_move_bottom),
    ("scroll-up", do_scroll_up),
    ("scroll-down", do_scroll_down),
    ("move-begin-line", do_move_begin_line),
    ("move-end-line", do_move_end_line),
    ("redraw", do_redraw),
];

fn init_edit_map() -> EditMap {
    let mut edit_map = EditMap::new();
    for (op, edit) in EDIT_MAPPINGS {
        edit_map.insert(op, edit);
    }
    edit_map
}

// Below is a collection of functions that get associated with canonocal names of editing
// operations.
//
// These functions are the glue between keys and editing operations, which are configurable
// and bound at runtime.

/// insert-char
fn do_insert_char(editor: &mut Editor, key: &Key) -> Result<()> {
    match key {
        Key::Char(c) => {
            editor.insert_char(*c);
            Ok(())
        }
        _ => Err(format!("{key:?}: expecting Key::Char").into()),
    }
}

/// delete-char-left
fn do_delete_char_left(editor: &mut Editor, _: &Key) -> Result<()> {
    // todo: should we return deleted char in result?
    let _ = editor.delete_left();
    Ok(())
}

/// delete-char-right
fn do_delete_char_right(editor: &mut Editor, _: &Key) -> Result<()> {
    // todo: should we return deleted char in result?
    let _ = editor.delete_right();
    Ok(())
}

/// move-up
fn do_move_up(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_up();
    Ok(())
}

/// move-down
fn do_move_down(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_down();
    Ok(())
}

/// move-left
fn do_move_left(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_left();
    Ok(())
}

/// move-right
fn do_move_right(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_right();
    Ok(())
}

/// move-page-up
fn do_move_page_up(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_page_up();
    Ok(())
}

/// move-page-down
fn do_move_page_down(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_page_down();
    Ok(())
}

/// move-top
fn do_move_top(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_top();
    Ok(())
}

/// move-bottom
fn do_move_bottom(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_bottom();
    Ok(())
}

/// scroll-up
fn do_scroll_up(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.scroll_up();
    Ok(())
}

/// scroll-down
fn do_scroll_down(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.scroll_down();
    Ok(())
}

/// move-begin-line
fn do_move_begin_line(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_beg();
    Ok(())
}

/// move-end-line
fn do_move_end_line(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.move_end();
    Ok(())
}

/// redraw
fn do_redraw(editor: &mut Editor, _: &Key) -> Result<()> {
    editor.align_cursor(Focus::Auto);
    Ok(())
}
