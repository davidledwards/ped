# Changelog

This file contains relevant information for each release. Please refer to the commit history for more details.

### [0.34.1](https://github.com/davidledwards/ped/tree/v0.34.1) | `2025-11-07`

#### Fixed

- When finding the next match via `search-next`, particularly when following a prior `search-*` operation, the search was not starting from the prior match position but rather the beginning of the buffer

### [0.34.0](https://github.com/davidledwards/ped/tree/v0.34.0) | `2025-11-06`

#### Added

- Support for `S-tab` to step backwards through matches that accumulate during incremental searching when using `tab` to find next matches

### [0.33.0](https://github.com/davidledwards/ped/tree/v0.33.0) | `2025-11-03`

#### Added

- Support for `S-tab` in user input to traverse backwards through suggestions

### [0.32.0](https://github.com/davidledwards/ped/tree/v0.32.0) | `2025-11-02`

#### Fixed

- Subtle inconsistencies with interactive search

### [0.31.0](https://github.com/davidledwards/ped/tree/v0.31.0) | `2025-10-11`

#### Added

- Allow scrolling of editor windows via mouse when user input is active
- Configuration color `line-fg` which applies to highlighting current line number

### [0.30.0](https://github.com/davidledwards/ped/tree/v0.30.0) | `2025-10-04`

#### Changed

- Line numbers in left margin when exceeding space limit previously displayed `-----` but now print lower-order digits (ex: `--375` if line number was `218375`)
- Line and column numbers in banner bar now display up to `7` digits
- Reduced size of release binary ~30% by stripping debug symbols

### [0.29.0](https://github.com/davidledwards/ped/tree/v0.29.0) | `2025-08-16`

#### Added

- Search operations will initialize the term using selected text, but only if the text does not contain any control characters.
- When saving `@scratch` to a file, the contents of `@scratch` are now cleared.

### [0.28.1](https://github.com/davidledwards/ped/tree/v0.28.1) | `2025-07-19`

#### Fixed

- Partial key sequence being displayed in echo area after `redraw` operation

### [0.28.0](https://github.com/davidledwards/ped/tree/v0.28.0) | `2025-07-19`

#### Added

- Support for redrawing the entire workspace (via `redraw` bound to `M-l`) if the terminal output becomes mangled

### [0.27.0](https://github.com/davidledwards/ped/tree/v0.27.0) | `2025-07-12`

#### Added

- Ability to insert literal characters using Unicode code points in either _decimal_ or _hex_ format:
  - `insert-unicode-dec` bound to `C-^`
  - `insert-unicode-hex` bound to `M-^`

### [0.26.0](https://github.com/davidledwards/ped/tree/v0.26.0) | `2025-07-11`

#### Added

- Ability to interact with the clipboard provided by the native operating system:
  - `copy-global` bound to `M-c`
  - `paste-global` bound to `M-v`
  - `cut-global` bound to `M-x`

### [0.25.1](https://github.com/davidledwards/ped/tree/v0.25.1) | `2025-06-29`

#### Fixed

- Regression in handling key modifiers `S-` and `C-` that made some navigation keys unrecognizable

### [0.25.0](https://github.com/davidledwards/ped/tree/v0.25.0) | `2025-06-28`

#### Added

- Support for `INSERT` and `DELETE` keys with optional modifiers:
  - `insert`
  - `S-insert`
  - `C-insert`
  - `S-C-insert`
  - `delete`
  - `S-delete`
  - `C-delete`
  - `S-C-delete`

#### Changed

- Removed the following _restricted_ keys, which previously could not be rebound:
  - `C-h`
  - `C-i` or `tab`
  - `C-m` or `ret`
  - `C-?` or `del`
- Welcome prompt in echo area will now determine which key is bound to `help` and display that key rather than assuming `C-h`, which could be rebound

#### Fixed

- Terminal parsing was too forgiving, consequently accepting malformed ANSI sequences

### [0.24.0](https://github.com/davidledwards/ped/tree/v0.24.0) | `2025-04-19`

##### Changed

- Undo/Redo correctly restores _dirtiness_ state of buffer, i.e. undoing a series of edits starting from an _undirty_ buffer will restore the buffer to an _undirty_ state

### [0.23.0](https://github.com/davidledwards/ped/tree/v0.23.0) | `2025-03-30`

#### Fixed

- Panic in certain edge cases when writing to `stdout` and pipe is broken by receiver, e.g. `ped -h | echo`

### [0.22.0](https://github.com/davidledwards/ped/tree/v0.22.0) | `2025-02-17`

#### Changed

- Release process from shell script to GitHub Actions

### [0.21.0](https://github.com/davidledwards/ped/tree/v0.21.0) | `2025-02-16`

#### Added

- CLI option `--[no-]track-lateral` to enable or disable tracking of lateral mouse events
- Configuration option `track-lateral`

#### Changed

- Lateral mouse tracking is now _disabled_ by default

### [0.20.0](https://github.com/davidledwards/ped/tree/v0.20.0) | `2025-02-15`

#### Added

- CLI option `--describe` that prints the description of an operation

#### Changed

- `@help` window dynamically generates key bindings with descriptions rather than using static content
- `@operations` window shows _description_ in addition to name of operation
- TOML output for CLI option `--bindings`, `--colors`, and `--theme` now use single quote for strings to avoid escaping backslash
- `C-h` is now a restricted key that cannot be rebound

### [0.19.1](https://github.com/davidledwards/ped/tree/v0.19.1) | `2025-02-12`

#### Fixed

- Banner bar of the _active_ window was being redrawn with the _inactive_ background color when the terminal was resized

### [0.19.0](https://github.com/davidledwards/ped/tree/v0.19.0) | `2025-02-06`

#### Changed

- Show column number on banner bar up to `99999` before displaying `-----`
- Optimized display performance by reducing amount of ANSI sequences written to terminal

#### Fixed

- Cursor not being positioned correctly after resizing terminal

### [0.18.0](https://github.com/davidledwards/ped/tree/v0.18.0) | `2025-01-31`

#### Added

- CLI option `--theme` that prints the color theme in TOML format
- CLI option `--source` that prints the GitHub repository URL corresponding to the specific commit when building `ped`

#### Changed

- Several default key bindings aimed at consistency
  - `M-o t` → `M-o a` (`open-file-top`: open file in new window at top of workspace)
  - `M-o b` → `M-o e` (`open-file-bottom`: open file in new window at bottom of workspace)
  - `M-y t` → `M-y a` (`select-editor-top`: switch to editor in new window at top of workspace)
  - `M-y b` → `M-y e` (`select-editor-bottom`: switch to editor in new window at bottom of workspace)
  - `M-w t` → `M-w a` (`top-window`: move to window at top of workspace)
  - `M-w b` → `M-w e` (`bottom-window`: move to window at bottom of workspace)
- Default value of `echo-fg` color to `208`
- Output of CLI options `--bindings` and `--colors` to TOML format

#### Fixed

- Incorrect character display in `describe-editor` (`C-t`) when cursor is positioned at end of file

### [0.17.0](https://github.com/davidledwards/ped/tree/v0.17.0) | `2025-01-28`

#### Added

- Background color of banner bar changes based on _active_ or _inactive_
- Configuration colors `active-bg` and `inactive-bg` applicable to banner bar

#### Changed

- Replaced configuration color `banner-bg` with `active-bg`

#### Fixed

- Changing tab mode between _hard_ and _soft_ now applies to editor in focus rather than entire workspace
- Retention of previous search result, for purpose of continuation, is now associated with applicable editor rather than entire workspace

### [0.16.0](https://github.com/davidledwards/ped/tree/v0.16.0) | `2025-01-25`

#### Added

- Support for `M-` as shorthand for `ESC` when binding keys, i.e. `M-x` is equivalent to `ESC:x`

### [0.15.0](https://github.com/davidledwards/ped/tree/v0.15.0) | `2025-01-24`

#### Added

- Interactive rendering of editor canvas when entering line numbers in `goto-line`

#### Changed

- Name of default syntax configuration from `Default` to `Text`

#### Fixed

- Missing call to canvas rendering in `goto-line`

### [0.14.0](https://github.com/davidledwards/ped/tree/v0.14.0) | `2025-01-24`

#### Added

- Incremental search for normal and regular expression vaiants, where pressing `TAB` moves to the next match
- CLI options `--no-spotlight`, `--no-lines`, and `--no-eol` to disable features

#### Changed

- Case-sensitivity of search is now bound to distinct key sequences
  - `C-\`: normal search (case-insensitive)
  - `M-C-\`: normal search (case-sensitive)
  - `M-\`: regular expression search (case-insensitive)
  - `M-M-\`: regular expression search (case-sensitive)
- Default background color of selected text is now `88` to improve contrast
- `--spotlight` and `--lines` are now enabled by default

### [0.13.1](https://github.com/davidledwards/ped/tree/v0.13.1) | `2025-01-20`

#### Added

- Single-character variants for some CLI options
- Support for `--` CLI option to forcibly stop interpretation of further options

### [0.13.0](https://github.com/davidledwards/ped/tree/v0.13.0) | `2025-01-19`

#### Added

- Enforcement of readonly editors, particularly ephemerals such as `@help`
- Syntax highlighting of help editors: `@help`, `@keys`, `@operations`, `@bindings`, and `@colors`

### [0.12.1](https://github.com/davidledwards/ped/tree/v0.12.1) | `2025-01-12`

#### Changed

- Tokenization of buffer for syntax highlighting moved to background processing

### [0.12.0](https://github.com/davidledwards/ped/tree/v0.12.0) | `2025-01-07`

#### Added

- Key binding `C-t` to show position and size of editor, including Unicode value of the character under the cursor
- Key binding `M-t t` to toggle between _soft_ and _hard_ tab inserts

#### Fixed

- Control characters other than `\n` and `\t` are now shown as `¿` with a dimmed foreground

### [0.11.0](https://github.com/davidledwards/ped/tree/v0.11.0) | `2025-01-05`

#### Added

- CLI option `--colors` to print color names and values
- Key binding `M-h c` to open `@colors` window
- Custom color names in addition to ANSI standard colors

#### Changed

- Default color theme

### [0.10.0](https://github.com/davidledwards/ped/tree/v0.10.0) | `2025-01-05`

#### Added

- Show applicable syntax configuration in banner bar
- Configuration color `accent-fg` to enhance banner bar

#### Changed

- Progressive layout of banner bar based on width of terminal
- File completion is now case-insensitive when matching candidates

#### Fixed

- Restoring terminal properly under certain failure cases at startup

### [0.9.0](https://github.com/davidledwards/ped/tree/v0.9.0) | `2025-01-02`

#### Added

- CLI options `--tab-hard` and `--tab-soft` to insert literal `\t` or spaces, respectively, when `TAB` key is pressed

#### Changed

- CLI option `--tab` to `--tab-size`
- Configuration setting `tab` to `tab-size`
- Configuration color `eol-fg` to `whitespace-fg`

#### Fixed

- Rendering of `\t` now correctly shows single character `→` (does not indent as one might expect)

### [0.8.1](https://github.com/davidledwards/ped/tree/v0.8.1) | `2024-12-28`

#### Fixed

- Panic when deleting to the end of buffer _and_ text is being tokenized for syntax highlighting

### [0.8.0](https://github.com/davidledwards/ped/tree/v0.8.0) | `2024-12-27`

#### Added

- CLI options `--bare` and `--bare-syntax` to ignore loading, respectively, _all_ configuration files or syntax configurations only

#### Changed

- Syntax configuration files now use regular expressions to match against file names rather than prior method of static file extensions (see [ped-syntax](https://github.com/davidledwards/ped-syntax))

### [0.7.0](https://github.com/davidledwards/ped/tree/v0.7.0) | `2024-12-27`

#### Changed

- Shortened names of CLI options
  - `--show-spotlight` → `--spotlight`
  - `--show-lines` → `--lines`
  - `--show-eol` → `--eol`
  - `--tab-size` → `--tab`
  - `--print-keys` → `--keys`
  - `--print-ops` → `--ops`
  - `--print-bindings` → `--bindings`
  - `--syntax-dir` → `--syntax`
- Shortened names of configuration settings
  - `show-spotlight` → `spotlight`
  - `show-lines` → `lines`
  - `show-eol` → `eol`
  - `tab-size` → `tab`

### [0.6.1](https://github.com/davidledwards/ped/tree/v0.6.1) | `2024-12-26`

#### Added

- Selection via `SHIFT` key when scrolling by mouse

#### Changed

- Minor changes to default key bindings for scrolling and moving forward and backward by word
- Major usability improvement when referencing and defining colors

#### Fixed

- Mouse tracking now _scrolls_ instead of moving cursor

### [0.6.0](https://github.com/davidledwards/ped/tree/v0.6.0) | `2024-12-26`

#### Changed

- Major usability improvement when referencing and defining colors

#### Fixed

- Minor display issues related to prior method of color management

### [0.5.0](https://github.com/davidledwards/ped/tree/v0.5.0) | `2024-12-24`

#### Added

- Syntax highlighting
- Additional location to search for configuration file

### [0.4.0](https://github.com/davidledwards/ped/tree/v0.4.0) | `2024-12-11`

#### Added

- New key bindings to open existing editors in new windows

#### Changed

- C-SPACE now _unsets_ an active mark
- Default selection color to improve readability

### [0.3.1](https://github.com/davidledwards/ped/tree/v0.3.1) | `2024-12-10`

#### Fixed

- Mouse scrolling now applies to editor where cursor is focused

### [0.3.0](https://github.com/davidledwards/ped/tree/v0.3.0) | `2024-12-09`

#### Added

- Use alternate screen buffer to preserve terminal history
- Recognize mouse scroll events as keys bound to navigation
- Recognize button events to change editor focus and set cursor position

### [0.2.0](https://github.com/davidledwards/ped/tree/v0.2.0) | `2024-12-06`

#### Added

- Search using string
- Search using regular expression

### [0.1.0](https://github.com/davidledwards/ped/tree/v0.1.0) | `2024-12-02`

#### Added

- Initial release of functional editor
- Multiple windows
- Multiple buffers
- Cut, copy, paste
- Undo, redo
- File completion
- Line numbers
- Key binding at runtime
- Configurable colors
- Help
- Useful features notably absent - search, syntax coloring, themes, mouse events
