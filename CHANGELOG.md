# Changelog

This file contains relevant information for each release. Please refer to the commit history for more details.

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
