//! Main controller.

use crate::editor::{Editor, Focus};
use crate::error::Result;
use crate::key::{Key, Keyboard, Modifier};
use crate::workspace::Workspace;

use std::collections::HashMap;

//
// this is the controller that loops indefinitely over keystrokes, modifies the buffer,
// and updates the display.
//
// display:
// - represent the full terminal space, i.e. what can be displayed to the user
//
// window:
// - this is the actual writeable space on the terminal
// - could have many windows on a single terminal
// - bounded by the display size
// - we assume windows are tiled, which means they do not share the same terminal space
//
// ideas:
// - we can use a window not only for text editing, but also prompting
// - prompting would be like a 1-line editor with no wrapping, no formatting, where <return>
//   would terminate the editing session
//
// organization:
// - display (1)
//   - window (n)
// - windows are organized as tiles on display
// - tiling could be done in descriptive manner, e.g. using relative positions, which allows
//   the display to adjust window positions when the display size changes
//
// state we need:
// - buffer
// - display
// - tty
// - origin (line, col) of window on display
// - current (line, col) of position in the buffer: assumption is that the buffer is only
//   responsible for maintaining the gap range, which essentially gives us the index.
// - current (row, col) of cursor in displau: this could be maintained by the display itself,
//   though it will depend on where it is easiest to maintain state.
//
// basic flow:
// - loop over keystrokes indefinitely
// - key is either movement or change
//   - a movement may impact the display or just move the cursor
//   - a change will always modify the buffer and impact the display
// - we can view movement as a no-op change (no mod to buffer)
// - change:
//   - normal case is single char insertion
//   - general case is n-char insertion or deletion
//   - a cut/paste can be seen as two changes, deletion + insertion
// - processing:
//   - update buffer
//   - determine how change impacts the display
//     - cases to consider:
//       - shift line left or right only
//       - wraps to next line, possibly n lines depending on length of line in buffer
//         - e.g. 200-char line in buffer, but 80-col window, means buffer line is displayed
//           on 3 display lines
//       - same case of n-line wrapping when removing text
//   - send commands to display
//   - set cursor on display
// - modes:
//   - insert or overwrite
// - display modes:
//   - wrap
//   - no-wrep (shifts text left/right)
// - formatting modes:
//   - none
//   - token-based formatter, e.g. regex terms
//   - formatting only needs to consider what's actually being displayed, not the entire
//     buffer, though this does require additional CPU for JIT formmating
// - syntax highlighting or computationally-expensive formatting
//   - consider using the keyboard loop when a timeout occurs to reformat. this could be useful
//     for things that require enough CPU which could lead to sluggish user response.
// - repainting
//   - consider a map of line number to buffer index to assist in the repainting process.
//   - would need to update these mappings when the buffer changes, so adds complexity and
//     computation.
//
// could be > 1 window attached to buffer. editor makes updates to the buffer, then sends the
// update to each window for paint consideration.
// - a single window has focus, which means it has the cursor
// - another window could also be attached and may have the portion of the buffer being edited
//   visible. however, we do not want to scroll shift the window since its not in focus.
// - e.g. window in focus may need to scroll up/down if insertion causes the cursor to shift
//   outside the visible area, so that requires some kind of scroll. the window not in focus
//   may be visible, but should not scroll. rather, it should simply ignore the update.
// - what this boils down to is that a window not in focus anchors its window to a position in
//   the buffer. if anything in the buffer changes with respect to the visible area based on
//   the anchored position, then it the window will make updates as necessary.
//
// navigation only:
// - note that movement is relative to the window.
// - a cursor-down does not mean a literal move from (x, y) to (x, y + 1). rather, it means
//   1 visual line down, which could be on the same logical line if the line wraps at the end.
// - if the window is in no-wrap mode, then it is same as moving +1 logical line.
// - page-up means backtracking one visual page. do we ask the window to page-up by finding
//   the prior page origin (top x,y) and cursor?
//
// editor has list of windows (editing contexts). a tile from the workspace must be attached
// to a window in order for the user to interact with it. when it is detached, the window
// no longer becomes visible, yet the editing context is still retained by the editor.
//

// key map concept:
// - map of "action" -> function
//   - example: "cursor-up" -> move_cursor_up()
//   - essentially a static map
//   - used to build the actual key map at runtime
//
// - map of Key -> function
//   - example: Key::Control(1) -> move_beg_of_line()
//   - gets built at runtime
//   - keys are well-known and finite, used to drive construction of the map
//
// struct KeyName {
//    id: &'static str,
//    key: Key,
// }
//
// key map is essentially: KeyMap<KeyName, Fn>
//
// construct the map by iterating through array of KeyName, find key.id in the action
// map, which returns a Fn, then add (key.key, fn) to the key map.
// - if any key.id is not found, then panic since this would indicaate an inconsistent
//   state.
//
// in practice, a user may want to rebind a "key" to an "action". for example, a user may
// change the keys used for moving to beg and end of line.
// ^A -> "move-beg-of-line"
// ^E -> "move-end-of-line"
//
// these would be loaded from an external file and bound at runtime using the same
// method described above.
//
// what do we bind by default? should this be externalized? or, embedded in the code?
// it seems we would want a default keymap in case the externalized bindings could not
// be located at runtime.
//
// KeyMap::new() -> KeyMap -- creates default keymap
// KeyMap::load(file) -> KeyMap -- create keymap using bindings from file
//
// key::keys -> &'static [KeyName]

type KeyMap = HashMap<Key, Box<dyn Fn(&mut Editor) -> Result<()>>>;

pub struct Controller {
    keyboard: Keyboard,
    workspace: Workspace,
    editor: Editor,
    keymap: KeyMap,
}

fn do_move_beg(editor: &mut Editor) -> Result<()> {
    editor.move_beg();
    Ok(())
}

fn do_move_down(editor: &mut Editor) -> Result<()> {
    editor.move_down();
    Ok(())
}

impl Controller {
    pub fn new(keyboard: Keyboard, workspace: Workspace, editor: Editor) -> Controller {
        let mut keymap: KeyMap = HashMap::new();
        keymap.insert(Key::Control(1), Box::new(do_move_beg));
        keymap.insert(Key::Down(Modifier::None), Box::new(do_move_down));

        Controller {
            keyboard,
            workspace,
            editor,
            keymap,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            match self.keyboard.read()? {
                // ctrl-X
                Key::Control(24) => break,
                Key::None => {
                    // check for change in terminal size and update workspace
                }
                Key::Up(Modifier::None) => {
                    self.editor.move_up();
                }
                Key::Down(Modifier::None) => {
                    self.editor.move_down();
                }
                Key::Left(Modifier::None) => {
                    self.editor.move_left();
                }
                Key::Right(Modifier::None) => {
                    self.editor.move_right();
                }
                // fn/up-arrow
                Key::PageUp(Modifier::None) => {
                    self.editor.move_page_up();
                }
                // fn/down-arrow
                Key::PageDown(Modifier::None) => {
                    self.editor.move_page_down();
                }
                // fn/left-arrow
                Key::Home(Modifier::None) => {
                    self.editor.move_top();
                }
                // fn/right-arrow
                Key::End(Modifier::None) => {
                    self.editor.move_bottom();
                }
                Key::Up(Modifier::ShiftControl) => {
                    self.editor.scroll_up();
                }
                Key::Down(Modifier::ShiftControl) => {
                    self.editor.scroll_down();
                }
                // ctrl-A
                Key::Control(1) => {
                    self.editor.move_beg();
                }
                // ctrl-E
                Key::Control(5) => {
                    self.editor.move_end();
                }
                // ctrl-L
                Key::Control(12) => {
                    self.editor.align_cursor(Focus::Auto);
                }
                // ctrl-R
                Key::Control(18) => {
                    self.editor.render();
                }
                // "1"
                Key::Char('1') => {
                    let cs = "^lorem-ipsum$".chars().collect();
                    self.editor.insert_chars(&cs);
                }
                // "2"
                Key::Char('2') => {
                    let cs = "^lorem-ipsum$\n^lorem-ipsum$\n^lorem-ipsum$"
                        .chars()
                        .collect();
                    self.editor.insert_chars(&cs);
                }
                // "3"
                Key::Char('3') => {
                    let cs = "@".repeat(10000).chars().collect();
                    self.editor.insert_chars(&cs);
                }
                // "6"
                Key::Char('6') => {
                    let (_, cur_pos) = self.editor.cursor();
                    let _ = self.editor.remove_from(cur_pos.saturating_sub(10));
                }
                // "7"
                Key::Char('7') => {
                    let (_, cur_pos) = self.editor.cursor();
                    let _ = self.editor.remove_to(cur_pos + 10);
                }
                // backspace
                Key::Backspace => {
                    let _ = self.editor.delete_left();
                }
                // ctrl-D
                Key::Control(4) => {
                    let _ = self.editor.delete_right();
                }
                Key::Char(c) => {
                    self.editor.insert_char(c);
                }
                _ => {}
            }
        }
        Ok(())
    }
}
