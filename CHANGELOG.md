# Changelog

This file contains relevant information for each release. Please refer to the commit history for more details.

## [0.9.0](https://github.com/davidledwards/ped/tree/v0.9.0) - `2025-01-02`

### Added

- CLI options `--tab-hard` and `--tab-soft` to insert literal `\t` or spaces, respectively, when `TAB` key is pressed

### Changed

- CLI option `--tab` to `--tab-size`
- Configuration setting `tab` to `tab-size`
- Configuration color `eol-fg` to `whitespace-fg`

### Fixed

- Rendering of `\t` now correctly shows single character `→` (does not indent as one might expect)

## [0.8.1](https://github.com/davidledwards/ped/tree/v0.8.1) - `2024-12-28`

### Fixed

- Panic when deleting to the end of buffer _and_ text is being tokenized for syntax highlighting

## [0.8.0](https://github.com/davidledwards/ped/tree/v0.8.0) - `2024-12-27`

### Added

- CLI options `--bare` and `--bare-syntax` to ignore loading, respectively, _all_ configuration files or syntax configurations only

### Changed

- Syntax configuration files now use regular expressions to match against file names rather than prior method of static file extensions (see [ped-syntax](https://github.com/davidledwards/ped-syntax))

## [0.7.0](https://github.com/davidledwards/ped/tree/v0.7.0) - `2024-12-27`

### Changed

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

## [0.6.1](https://github.com/davidledwards/ped/tree/v0.6.1) - `2024-12-26`

### Added

- Selection via `SHIFT` key when scrolling by mouse

### Changed

- Minor changes to default key bindings for scrolling and moving forward and backward by word
- Major usability improvement when referencing and defining colors

### Fixed

- Mouse tracking now _scrolls_ instead of moving cursor

## [0.6.0](https://github.com/davidledwards/ped/tree/v0.6.0) - `2024-12-26`

### Changed

- Major usability improvement when referencing and defining colors

### Fixed

- Minor display issues related to prior method of color management

## [0.5.0](https://github.com/davidledwards/ped/tree/v0.5.0) - `2024-12-24`

### Added

- Syntax highlighting
- Additional location to search for configuration file

## [0.4.0](https://github.com/davidledwards/ped/tree/v0.4.0) - `2024-12-11`

### Added

- New key bindings to open existing editors in new windows

### Changed

- C-SPACE now _unsets_ an active mark
- Default selection color to improve readability

## [0.3.1](https://github.com/davidledwards/ped/tree/v0.3.1) - `2024-12-10`

### Fixed

- Mouse scrolling now applies to editor where cursor is focused

## [0.3.0](https://github.com/davidledwards/ped/tree/v0.3.0) - `2024-12-09`

### Added

- Use alternate screen buffer to preserve terminal history
- Recognize mouse scroll events as keys bound to navigation
- Recognize button events to change editor focus and set cursor position

## [0.2.0](https://github.com/davidledwards/ped/tree/v0.2.0) - `2024-12-06`

### Added

- Search using string
- Search using regular expression

## [0.1.0](https://github.com/davidledwards/ped/tree/v0.1.0) - `2024-12-02`

### Added

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
