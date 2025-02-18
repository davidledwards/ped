# ped

The *ped*estrian text editor.

<p align="center">
    <img src="content/ped.png" width="25%" height="25%" />
</p>

![](content/ped-snapshot.png)

## Background

The genesis of this project stemmed from a desire to learn the [Rust](https://www.rust-lang.org/) programming language while also building something useful and nontrivial.

This is a hobbyist project with the goal of building a fully functional terminal-based editor supporting multiple buffers and windows, regular expression search, syntax highlighting, and perhaps other features yet to be decided. I plan to develop most everything from scratch as opposed to using prebuilt libraries. The desire is to learn, not to go fast.

An [evolving narrative](DESIGN.md) of the editor explores many of the design choices in greater detail.

## Install

### Install from Homebrew

```shell
brew tap davidledwards/ped
brew install ped
```

### Install from GitHub

Releases can be downloaded directly from [GitHub](https://github.com/davidledwards/ped/releases).

Alternatively, using the [GitHub CLI](https://cli.github.com/), releases can also be downloaded from the command line. For example, to download version `0.22.0`, run the following command.

```shell
gh release download --repo https://github.com/davidledwards/ped v0.22.0
```

## Usage

Run `ped --help` (or `ped -h`) to print a description of all available options.

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

Alternatively, a configuration file can be specified on the command line using the `--config` (or `-C`) option.

```shell
ped --config ~/alt/.pedrc README.md
```

See [.pedrc](.pedrc) for a detailed explanation of configuration settings. In the absence of a configuration file, `ped` will rely on default values.

`ped` will also try to locate syntax configuration files in one of the following directories in order of precedence. See the [ped-syntax](https://github.com/davidledwards/ped-syntax) repository for more information about creating and installing syntax files.

- `$HOME/.ped/syntax`
- `$HOME/.config/ped/syntax`

Alternatively, a directory containing syntax configurations can be specified using the `--syntax` (or `-S`) option.

```shell
ped --syntax ~/alt/syntax README.md
```

`ped` can also be instructed to ignore all configuration files, including syntax configurations, using the `--bare` (or `-b`) and `--bare-syntax` (or `-B`) options, respectively. This is useful in circumstances where configuration files contain errors, which will cause `ped` to exit prematurely.

This ignores _all_ configurations.

```shell
ped --bare README.md
```

This ignores syntax configurations only. Note that the configuration file is still loaded.

```shell
ped --bare-syntax README.md
```

## Keys

The editor comes prebuilt with a default set of opinionated key bindings. Until the project reaches version `1.0`, these builtin key bindings may change.

These key bindings can be enumerated from the CLI, the output of which may be included in a configuration file under the `[bindings]` section. See [.pedrc](.pedrc) for more details on modifying key bindings.

Prints the key bindings.

```shell
ped --bindings
```

Note that the prior command also reflects any modifications from a configuration file. This alternative command will print default bindings only.

```shell
ped --bare --bindings
```

Prints a list of available keys that can be used in constructing key sequences.

```shell
ped --keys
```

Prints a list of available operations that to which keys can be bound.

```shell
ped --ops
```

Prints a brief description of the `undo` operation.

```shell
ped --describe undo
```

While running `ped`, typing `C-h` will show general help information that also contains a list of all key bindings. In other words, the same content as `ped --bindings`.

## Colors

The color mechanism in `ped` adheres to the [ANSI 8-bit color standard](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit). Where applicable in configuration files, colors can always be referred to by their numeric value in the range of `0` to `255`.

However, in order to make configuration a bit more friendly, `ped` defines a set of builtin names for standard colors in the range of `0` to `15`, as well as a few creatively named colors in the extended range of `16` to `255`. The extended colors are likely to evolve over time, but there is no intention of producing an exhaustive list of names.

Prints a list of color names.

```shell
ped --colors
```

Note that the prior command also shows any color additions or modifications from a configuration file. The following command only prints builtin colors.

```shell
ped --bare --colors
```

Prints the current color theme settings, the output of which can be included in a configuration file under the `[theme]` section.

```shell
ped --theme
```

Since the prior command reflects any theme modifications from a configuration file, the following shows only the default theme settings.

```shell
ped --bare --theme
```

In the course of experimenting with different color combinations, I decided to write a CLI program [tcolor](https://github.com/davidledwards/tcolor) that shows what colors look like on the terminal. The program is quite simple but also effective in visually colors.

## Limitations

`ped` currently has a few notable limitations or deficiencies that may be addressed in future versions.

- An auto-save feature has not been implemented, so remember to save frequently.
- TAB characters `\t` are not indented as one might expect, but rather shown as the special character `→`.
- Control characters other than `\t` and `\n` are shown as `¿`, though one can place the cursor under such characters and press `C-t` to see the Unicode code point.
- A target binary does not exist for Windows.

## Release

Instructions for building and releasing can be found [here](RELEASE.md).

## Contributing

Please refer to the [contribution guidelines](CONTRIBUTING.md) when reporting bugs and suggesting improvements.

## License

Copyright 2024 David Edwards

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at

<http://www.apache.org/licenses/LICENSE-2.0>

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.
