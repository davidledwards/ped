//! Manages workspace.

use crate::canvas::Point;
use crate::color::Color;
use crate::error::Result;
use crate::term;
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
// Document: an editing context attached to a buffer
// - file path
// - buffer
// - cursor (row, col)
// - list of windows (possibly none if not visible on workspace)
// - window (window where editing is focused, possibly none)
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
// Editor: a controller that allows editing of documents in the workspace
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

pub struct Workspace {
    rows: u32,
    cols: u32,
}

impl Workspace {
    pub fn new() -> Result<Workspace> {
        let (rows, cols) = term::size()?;
        Ok(Workspace { rows, cols })
    }

    // temp for now, just to give us a window occupying the workspace
    pub fn new_window(&mut self) -> Window {
        Window::new(
            Point::new(0, 0),
            self.rows - 1,
            self.cols,
            Color::new(46, 232),
        )
    }
}
