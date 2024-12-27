# ped

The *ped*estrian text editor.

![](content/ped-snapshot.png)

## Background

The genesis of this project stemmed from a desire to learn the [Rust](https://www.rust-lang.org/) programming language while also building something useful and nontrivial.

During my university years studying computer science, I spent evenings and weekends developing a text editor for MS-DOS in Turbo Pascal 3.0. Unfortunately, the source code seems to have been lost forever, but rest assured that reviewing the code today would likely be a dreadful experience.

This is a hobbyist project with the goal of building a fully functional terminal-based editor supporting multiple buffers and windows, regular expression search, syntax highlighting, and perhaps other features yet to be decided. I plan to develop most everything from scratch as opposed to using prebuilt libraries. The desire is to learn, not to go fast.

## Install

### Install from Homebrew

```shell
brew tap davidledwards/ped
brew install ped
```

### Install from GitHub

Releases can be downloaded directly from [GitHub](https://github.com/davidledwards/ped/releases).

Alternatively, using the [GitHub CLI](https://cli.github.com/), releases can also be downloaded from the command line. For example, to download version `0.8.0`, run the following command.

```shell
gh release download --repo https://github.com/davidledwards/ped v0.8.0
```

## Usage

Run `ped --help` to print a description of all available options.

Edit a file.

```shell
ped foo.rs
```

Edit multiple files, opened in separate windows.

```shell
ped foo.rs bar.rs
```

`ped` will try to locate and read a configuration file at one of the following paths in order of precedence.

- `$HOME/.pedrc`
- `$HOME/.ped/pedrc`
- `$HOME/.config/ped/pedrc`

Alternatively, a configuration file can be specified on the command line using the `--config` option.

```shell
ped --config ~/alt/.pedrc README.md
```

See [.pedrc](.pedrc) for a detailed explanation of configuration settings. In the absence of a configuration file, `ped` will rely on default values.

`ped` will also try to locate syntax configuration files in one of the following directories in order of precedence. See the [ped-syntax](https://github.com/davidledwards/ped-syntax) repository for more information about creating and installing syntax files.

- `$HOME/.ped/syntax`
- `$HOME/.config/ped/syntax`

Alternatively, a directory containing syntax configurations can be specified using the `--syntax` option.

```shell
ped --syntax ~/alt/syntax README.md
```

`ped` can also be instructed to ignore all configuration files, including syntax configurations, using the `--bare` and `--bare-syntax` options, respectively. This is useful in circumstances where configuration files contain errors, which will cause `ped` to exit prematurely.

This ignores _all_ configurations.

```shell
ped --bare README.md
```

This ignores syntax configurations only. Note that the configuration file is still loaded.

```shell
ped --bare-syntax README.md
```

## Tour

The following notation is used below when refering to keys.

- `C-<key>` means `CONTROL` + `<key>`
- `S-<key>` means `SHIFT` + `<key>`
- `S-C-<key>` means `SHIFT` + `CONTROL` + `<key>`
- `M-<key>` means `ESCAPE` (or `META`) + `<key>`

### General

| Key   | Command             |
| ----- | ------------------- |
| `C-q` | Quit ped            |
| `C-g` | Cancel command      |
| `C-h` | Toggle @help window |

### Navigation

| Key              | Command                     |
| ---------------- | --------------------------- |
| `C-b` `←`        | Move backward one character |
| `C-f` `→`        | Move forward one character  |
| `C-p` `↑`        | Move up one line            |
| `C-n` `↓`        | Move down one line          |
| `C-a` `HOME`     | Move to start of line       |
| `C-e` `END`      | Move to end of line         |
| `M-p` `PAGEUP`   | Move up one page            |
| `M-n` `PAGEDOWN` | Move down one page          |
| `M-a` `C-HOME`   | Move to top of editor       |
| `M-e` `C-END`    | Move to end of editor       |
| `M-b` `C-←`      | Move backward one word      |
| `M-f` `C-→`      | Move forward one word       |
| `C-↑`            | Scroll up one line          |
| `C-↓`            | Scroll down one line        |
| `C-/`            | Go to line number           |

### Editing

| Key   | Command                                        |
| ----- | ---------------------------------------------- |
| `RET` | Insert line break                              |
| `DEL` | Remove character before cursor                 |
| `C-d` | Remove character after cursor                  |
| `C-j` | Remove characters from start of line to cursor |
| `C-k` | Remove characters from cursor to end of line   |
| `C-u` | Undo last change                               |
| `C-r` | Redo last change                               |

### Selection

| Key       | Command                                     |
| --------- | ------------------------------------------- |
| `C-SPACE` | Set/Unset mark                              |
| `C-c`     | Copy selection or line to clipboard         |
| `C-x`     | Cut selection or line and copy to clipboard |
| `C-v`     | Paste contents of clipboard                 |

### Search

| Key   | Command                         |
| ----- | ------------------------------- |
| `C-\` | Search using string             |
| `M-\` | Search using regular expression |
| `C-]` | Search for next match           |

### Files

| Key     | Command                                        |
| ------- | ---------------------------------------------- |
| `C-o`   | Open file in current window                    |
| `M-o t` | Open file in new window at top of workspace    |
| `M-o b` | Open file in new window at bottom of workspace |
| `M-o p` | Open file in new window above current window   |
| `M-o n` | Open file in new window below current window   |
| `C-s`   | Save file                                      |
| `M-s`   | Save file as another name                      |

### Editors

| Key     | Command                                               |
| ------- | ----------------------------------------------------- |
| `C-y`   | Switch to editor in current window                    |
| `M-y t` | Switch to editor in new window at top of workspace    |
| `M-y b` | Switch to editor in new window at bottom of workspace |
| `M-y p` | Switch to editor in new window above current window   |
| `M-y n` | Switch to editor in new window below current window   |
| `M-,`   | Switch to previous editor in current window           |
| `M-.`   | Switch to next editor in current window               |

### Windows

| Key           | Command                               |
| ------------- | ------------------------------------- |
| `C-l`         | Redraw window and center cursor       |
| `C-w`         | Close window and editor               |
| `M-w 0`       | Close window                          |
| `M-w 1`       | Close all other windows               |
| `M-w t`       | Move to window at top of workspace    |
| `M-w b`       | Move to window at bottom of workspace |
| `M-w p` `M-<` | Move to window above current window   |
| `M-w n` `M->` | Move to window below current window   |

### Help

| Key     | Command                                          |
| ------- | ------------------------------------------------ |
| `C-h`   | Toggle @help window (general help)               |
| `M-h k` | Toggle @keys window (available keys)             |
| `M-h o` | Toggle @operations window (available operations) |
| `M-h b` | Toggle @bindings window (key bindings)           |

## Design

The core data structure for managing text is a [gap buffer](https://en.wikipedia.org/wiki/Gap_buffer) defined in `buffer.rs`, which turns out to be very efficient for insertion and removal operations. This is the only module that contains _unsafe_ Rust by design, primarily because the data structure requires something similar to a `Vec`, which could have been used but would have been too restrictive and less efficient. The simple idea behind the gap buffer that makes insertion and removal so efficient, _O(1)_, is that as the cursor moves so does the text before and after the gap. In essence, the cursor always points to the start of the gap, making insertion and removal a constant-time operation. This implementation has been slightly modified to defer any movement of text until a mutating change occurs.

The only other module that contains _unsafe_ Rust, by necessity, is `term.rs`, which makes calls to the C runtime library to interact with the terminal.

The display of text on the terminal is ultimately done using ANSI control sequences, but there are intermediate steps in the process that optimize the amount of data sent to the terminal. A key component of the display architecture is a _canvas_ that is essentially an abstraction over _stdout_. Central to the design of the canvas is the combination of a _front_ and _back_ grid, a two-dimensional data structure. The front grid is a faithful representation of what the user sees, whereas the back grid is a cache of pending updates. The idea is that a series of writes are applied to the back grid, and then a subsequent draw request will generate a minimal set of ANSI commands based on the differences between the front and back grids.

The rendering process is a possibly novel approach, as I did zero research on existing methods implemented in other editors. Since the text buffer is not organized as a collection of lines, but rather a contiguous array of characters with a gap in the middle, efficient rendering turned out to be one of the more difficult problems to solve. These core challenges include scrolling, insertion and removal of text, and line wrapping among others. It became evident early in design iterations that the rendering algorithms could be kept simple by only concerning themselves with the text visible on the display. That may seem obvious, but it is not necessarily intuitive when thinking through possible solutions. Notably, the rendering algorithm is based on two critical reference points in the buffer: one representing the top line of the display, and the other representing the line of the cursor. All movement and mutating operations are relative to these two points of reference. An earlier version used only one reference point, the current line, but it became clear that tracking the top line made a number of operations more efficient. The tradeoff with this algorithmic approach is that the majority of movement operations are _O(n)_, but in practice, these tend to be small distances, so the complexity that would come with implementing something more efficient than _O(n)_ would be hard to justify.

A _keyboard_ abstraction encapsulates the terminal, which is switched to _raw_ mode as part of initialization. The job of the keyboard is to interpret ANSI control sequences read from _stdin_ and turn those into _keys_. A key, or a sequence of keys, is bound to some editing operation, whether it be the simple insertion of a character or something more complex such as pasting text from the clipboard. In order to make the association between _key sequence_ and _editing operation_ more flexible, this binding process happens at runtime using a finite set of key names and editing operations. While `ped` does provide default bindings, these can be altered through configuration files.

An _editor_ is perhaps one of the more complicated data structures that combines a _buffer_ and a _window_. The purpose of the editor is to implement editing primitives that modify the underlying buffer and then determine how those changes are rendered in the window. The editing operations, which are bound to keys at runtime, are actually defined outside of the _editor_ in `op.rs`. The idea is that all current and future operations can be built using the editor primitives.

The entire editing experience is facilitated by a central _controller_, which in a simplified sense, reads keys and calls their corresponding editing operations. The controller also manages the workspace, which contains a collection of windows, and provides a restricted _environment_ to functions that implement editing operations. It also coordinates interaction with the user in the form of _questions_, such as opening a file or asking to save a dirty buffer.

The concept of a _question_ is implemented using an _inquirer_ combined with a _completer_, both of which are abstractions that allow the controller to deal only with the general problem. This design allows the development of arbitrarily complex interactions, such as the _open file_ dialog that provides file completion assistance.

The _workspace_ supports multiple windows that split vertically with equal allocation of screen real estate. This was an early decision to keep the windowing system simple, at least for now. The workspace also manages resizing of windows when a change in the terminal size is detected.

## Release

Instructions for building and releasing can be found [here](RELEASE.md).

## Contributing

Please refer to the [contribution guidelines](CONTRIBUTING.md) when reporting bugs and suggesting improvements.

## License

Copyright 2024 David Edwards

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at

<http://www.apache.org/licenses/LICENSE-2.0>

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.
