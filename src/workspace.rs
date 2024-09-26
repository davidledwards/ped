//! Manages workspace.

use crate::canvas::Point;
use crate::color::Color;
use crate::error::Result;
use crate::window::Window;

// idea behind the workspace is that it manages the individual windows.
//
// the workspace would occupy the entire terminal, but that is dependent on the configuration.
// the editor would watch for changes in the terminal size and send update to the workspace.
//
// the workspace should manage the window sizes and placements.
// considerations:
// - create workspace using terminal size information.
// - workspace is empty by default, meaning no windows.
// - provide function to create window.
//
// what does the interface look like? the first window is obvious: it occupies the entire
// workspace. what about the second window? ideally, the workspace decides how to organize
// the windows. so, if a second window is created, then it divides the space in half. a third
// window would divide the space into thirds.
//
// the caller could give a hint to the workspace, such as bottom/top/left/right, which
// would indicate relative placement.
//
// no windows overlap, always tiled.
//
//
// Buffer: contains the actual text represented by a gap data structure
// - buf*
// - capacity
// - size
// - ...
//
// Editor: an editing context attached to a buffer
// - name/URL/path
// - buffer
// - cursor (row, col)
// - window (if attached, possibly none)
//
// Window: a viewable area on the workspace
// - origin (row, col)
// - rows
// - cols
// - back/front canvas
// - display (viewable area on terminal)
//
// Workspace: the collection of windows on the terminal
// - rows
// - cols
// - list of windows
//
// Controller: a controller that allows editing of documents in the workspace
// - workspace
// - list of buffers
// - list of documents (each document attached to a buffer)
// - active document (editing operations apply to this document)
// - list of windows (possibly many, each attached to a document)
//
// questions:
// - if more than 1 window is attached to the same document, how do we refresh the display of
//   windows that are not the active window? in other words, if an edit occurs in window A
//   for document "foo.rs", how does window B get updated, particularly if the edit in A is
//   also visible in B?
//

// map of (id, window)
// - take(id): returns window, which allows it to be attached to an editor
//
// struct Workspace
// - rows
// - cols
// - windows: map<id, window>
// - default color
//
// when new window is created, existing windows may need to be resized. this also happens
// if the terminal size changes and the workspace is asked to resize itself.
// - each window likely to get recreated since the size changes, and hence the canvas
//   must also change. in a sense, it would be akin to attaching a window to an editor.
// - for each impacted window, detach the editor, recreate the window, and reattach the
//   editor. this implies that a reference to the editor must be kept with the window.
//
// corner cases to consider
// - is there a minimum window size and workspace size?
// - what happens when adding a new window would violate the minimum window size?
// - how should the workspace behave under these circumstances?
//
// layout:
// - bottom row reserved for workspace
// -

const MAX_WINDOWS: usize = 32;

pub struct Workspace {
    rows: u32,
    cols: u32,
    windows: [Option<Window>; MAX_WINDOWS],
}

const FG_COLOR: u8 = 15;
// const FG_COLOR: u8 = 46;
const BG_COLOR: u8 = 233;

impl Workspace {
    pub fn new(rows: u32, cols: u32) -> Result<Workspace> {
        // create window to occupy workspace
        // must always have 1 window

        let mut windows = [const { None }; MAX_WINDOWS];
        let win = Window::new(
            Point::new(0, 0),
            rows - 1,
            cols,
            Color::new(FG_COLOR, BG_COLOR),
        );
        windows[0] = Some(win);
        Ok(Workspace {
            rows,
            cols,
            windows,
        })
    }

    pub fn window(&mut self, id: usize) -> Option<&mut Window> {
        if id < MAX_WINDOWS {
            self.windows[id].as_mut()
        } else {
            None
        }
    }

    pub fn open(&mut self) -> usize {
        // opens new window, should have argument instructing how this should be done.
        // a new window will always affect the dimensions of at least one window since
        // windows are tiled.
        //

        0
    }

    // temp for now, just to give us a window occupying the workspace
    pub fn new_window(&mut self) -> Window {
        Window::new(
            Point::new(0, 0),
            self.rows - 1,
            self.cols,
            Color::new(FG_COLOR, BG_COLOR),
        )
    }
}
