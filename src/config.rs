//! Contains everything related to configuration.
//!
//! All default values for configurable aspects of the editor are defined in this
//! module, including but not necessarily exclusive to _settings_, _colors_, and
//! _key bindings*_.
//!
//! At a minimum, [`Configuration::default()`] is sufficient for initializing the
//! editor. However, the normal process is to apply multiple tiers of configuration,
//! all optional, resulting in a final blended configuration.
//!
//! External configuration files are expected to be formatted according to the
//! [TOML specification](https://toml.io).
//!
//! The default method of loading an external configuration file via
//! [`Configuration::load()`] will try to locate files in the following locations in
//! order of precedence:
//!
//! * `$HOME/.pedrc`
//! * `$HOME/.ped/pedrc`
//! * `$HOME/.config/ped/pedrc`

use crate::bind::Bindings;
use crate::color::{ColorValue, Colors};
use crate::error::{Error, Result};
use crate::opt::Options;
use crate::syntax::Registry;
use crate::sys::{self, AsString};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::rc::Rc;

/// A configuration representing all aspects of the editing experience.
pub struct Configuration {
    /// A collection of configurable settings that control the behavior and rendering
    /// of editors.
    pub settings: Settings,

    /// A map of color names to color values.
    pub colors: Colors,

    /// A collection of configurable colors.
    pub theme: Theme,

    /// A map of key sequences to editing operations.
    pub bindings: Bindings,

    /// A registry of syntax configurations.
    pub registry: Registry,
}

pub type ConfigurationRef = Rc<Configuration>;

pub struct Settings {
    pub spotlight: bool,
    pub lines: bool,
    pub eol: bool,
    pub tab_hard: bool,
    pub tab_size: u32,
    pub track_lateral: bool,
}

pub struct Theme {
    pub text_fg: u8,
    pub text_bg: u8,
    pub select_bg: u8,
    pub spotlight_bg: u8,
    pub whitespace_fg: u8,
    pub accent_fg: u8,
    pub echo_fg: u8,
    pub prompt_fg: u8,
    pub banner_fg: u8,
    pub active_bg: u8,
    pub inactive_bg: u8,
    pub margin_fg: u8,
    pub margin_bg: u8,
    pub line_fg: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalConfiguration {
    settings: Option<ExternalSettings>,
    colors: Option<HashMap<String, u8>>,
    theme: Option<ExternalTheme>,
    bindings: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalSettings {
    spotlight: Option<bool>,
    lines: Option<bool>,
    eol: Option<bool>,

    #[serde(rename = "tab-hard")]
    tab_hard: Option<bool>,

    #[serde(rename = "tab-size")]
    tab_size: Option<u32>,

    #[serde(rename = "track-lateral")]
    track_lateral: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalTheme {
    #[serde(rename = "text-fg")]
    text_fg: Option<ColorValue>,

    #[serde(rename = "text-bg")]
    text_bg: Option<ColorValue>,

    #[serde(rename = "select-bg")]
    select_bg: Option<ColorValue>,

    #[serde(rename = "spotlight-bg")]
    spotlight_bg: Option<ColorValue>,

    #[serde(rename = "whitespace-fg")]
    whitespace_fg: Option<ColorValue>,

    #[serde(rename = "accent-fg")]
    accent_fg: Option<ColorValue>,

    #[serde(rename = "echo-fg")]
    echo_fg: Option<ColorValue>,

    #[serde(rename = "prompt-fg")]
    prompt_fg: Option<ColorValue>,

    #[serde(rename = "banner-fg")]
    banner_fg: Option<ColorValue>,

    #[serde(rename = "active-bg")]
    active_bg: Option<ColorValue>,

    #[serde(rename = "inactive-bg")]
    inactive_bg: Option<ColorValue>,

    #[serde(rename = "margin-fg")]
    margin_fg: Option<ColorValue>,

    #[serde(rename = "margin-bg")]
    margin_bg: Option<ColorValue>,

    #[serde(rename = "line-fg")]
    line_fg: Option<ColorValue>,
}

impl Settings {
    /// Applies the external settings `ext` on top of `self`.
    fn apply(&mut self, ext: Option<ExternalSettings>) {
        if let Some(ext) = ext {
            self.spotlight = ext.spotlight.unwrap_or(self.spotlight);
            self.lines = ext.lines.unwrap_or(self.lines);
            self.eol = ext.eol.unwrap_or(self.eol);
            self.tab_hard = ext.tab_hard.unwrap_or(self.tab_hard);
            self.tab_size = ext.tab_size.unwrap_or(self.tab_size);
            self.track_lateral = ext.track_lateral.unwrap_or(self.track_lateral);
        }
    }

    /// Applies the relevant settings from `opts` on top of `self`.
    pub fn apply_opts(&mut self, opts: &Options) {
        self.spotlight = opts.spotlight.unwrap_or(self.spotlight);
        self.lines = opts.lines.unwrap_or(self.lines);
        self.eol = opts.eol.unwrap_or(self.eol);
        self.tab_hard = opts.tab_hard.unwrap_or(self.tab_hard);
        self.tab_size = opts.tab_size.unwrap_or(self.tab_size);
        self.track_lateral = opts.track_lateral.unwrap_or(self.track_lateral);
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            spotlight: true,
            lines: true,
            eol: false,
            tab_hard: false,
            tab_size: 4,
            track_lateral: false,
        }
    }
}

impl Theme {
    const TEXT_FG: u8 = 252;
    const TEXT_BG: u8 = 233;
    const SELECT_BG: u8 = 88;
    const SPOTLIGHT_BG: u8 = 234;
    const WHITSPACE_FG: u8 = 243;
    const ACCENT_FG: u8 = 180;
    const ECHO_FG: u8 = 208;
    const PROMPT_FG: u8 = 102;
    const BANNER_FG: u8 = 254;
    const ACTIVE_BG: u8 = 60;
    const INACTIVE_BG: u8 = 237;
    const MARGIN_FG: u8 = 61;
    const MARGIN_BG: u8 = 234;
    const LINE_FG: u8 = 81;

    /// Applies the external theme `ext` on top of `self`.
    fn apply(&mut self, ext: Option<ExternalTheme>, colors: &Colors) -> Result<()> {
        fn resolve(color: u8, try_color: &Option<ColorValue>, colors: &Colors) -> Result<u8> {
            if let Some(try_color) = try_color {
                if let Some(color) = colors.lookup_value(try_color) {
                    Ok(color)
                } else {
                    Err(Error::invalid_color(&try_color.to_string()))
                }
            } else {
                Ok(color)
            }
        }

        if let Some(ext) = ext {
            self.text_fg = resolve(self.text_fg, &ext.text_fg, colors)?;
            self.text_bg = resolve(self.text_bg, &ext.text_bg, colors)?;
            self.select_bg = resolve(self.select_bg, &ext.select_bg, colors)?;
            self.spotlight_bg = resolve(self.spotlight_bg, &ext.spotlight_bg, colors)?;
            self.whitespace_fg = resolve(self.whitespace_fg, &ext.whitespace_fg, colors)?;
            self.accent_fg = resolve(self.accent_fg, &ext.accent_fg, colors)?;
            self.echo_fg = resolve(self.echo_fg, &ext.echo_fg, colors)?;
            self.prompt_fg = resolve(self.prompt_fg, &ext.prompt_fg, colors)?;
            self.banner_fg = resolve(self.banner_fg, &ext.banner_fg, colors)?;
            self.active_bg = resolve(self.active_bg, &ext.active_bg, colors)?;
            self.inactive_bg = resolve(self.inactive_bg, &ext.inactive_bg, colors)?;
            self.margin_fg = resolve(self.margin_fg, &ext.margin_fg, colors)?;
            self.margin_bg = resolve(self.margin_bg, &ext.margin_bg, colors)?;
            self.line_fg = resolve(self.line_fg, &ext.line_fg, colors)?;
        }
        Ok(())
    }
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            text_fg: Self::TEXT_FG,
            text_bg: Self::TEXT_BG,
            select_bg: Self::SELECT_BG,
            spotlight_bg: Self::SPOTLIGHT_BG,
            whitespace_fg: Self::WHITSPACE_FG,
            accent_fg: Self::ACCENT_FG,
            echo_fg: Self::ECHO_FG,
            prompt_fg: Self::PROMPT_FG,
            banner_fg: Self::BANNER_FG,
            active_bg: Self::ACTIVE_BG,
            inactive_bg: Self::INACTIVE_BG,
            margin_fg: Self::MARGIN_FG,
            margin_bg: Self::MARGIN_BG,
            line_fg: Self::LINE_FG,
        }
    }
}

impl Configuration {
    /// A collection of resource files to try loading in order of precedence.
    const TRY_FILES: [&str; 3] = [".pedrc", ".ped/pedrc", ".config/ped/pedrc"];

    /// Returns a configuration that is formed by attempting to load a resource file
    /// from well-known locations.
    pub fn load() -> Result<Configuration> {
        let mut config = Configuration::default();
        let root_path = sys::home_dir();
        for try_path in Self::TRY_FILES {
            let path = root_path.join(try_path);
            if path.exists() {
                let ext = Self::read_file(&path)?;
                config.apply(ext)?;
                break;
            }
        }
        Ok(config)
    }

    /// Returns a configuration loaded from the resource file at `path`.
    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<Configuration> {
        let mut config = Configuration::default();
        let ext = Self::read_file(path.as_ref())?;
        config.apply(ext)?;
        Ok(config)
    }

    /// Turns the configuration into a [`ConfigurationRef`].
    pub fn into_ref(self) -> ConfigurationRef {
        Rc::new(self)
    }

    /// Applies the relevant settings from `opts` on top of `self`.
    pub fn apply_opts(&mut self, opts: &Options) {
        self.settings.apply_opts(opts);
    }

    /// Applies the external configuration `ext` on top of `self`.
    fn apply(&mut self, ext: ExternalConfiguration) -> Result<()> {
        self.settings.apply(ext.settings);
        if let Some(colors) = ext.colors {
            self.colors.apply(&colors);
        }
        self.theme.apply(ext.theme, &self.colors)?;
        if let Some(bindings) = ext.bindings {
            for (key_seq, op) in bindings {
                self.bindings.bind(&key_seq, &op)?;
            }
        }
        Ok(())
    }

    fn read_file(path: &Path) -> Result<ExternalConfiguration> {
        let content = fs::read_to_string(path).map_err(|e| Error::io(&path.as_string(), e))?;
        toml::from_str::<ExternalConfiguration>(&content)
            .map_err(|e| Error::configuration(&path.as_string(), &e))
    }

    fn init_bindings() -> Bindings {
        let mut bindings = HashMap::new();
        for (key_seq, op) in Self::DEFAULT_BINDINGS {
            bindings.insert(key_seq.to_string(), op.to_string());
        }
        Bindings::new(&bindings).unwrap_or_else(|e| panic!("{e}: default bindings failed"))
    }

    #[rustfmt::skip]
    const DEFAULT_BINDINGS: [(&'static str, &'static str); 98] = [
        // --- exit and cancellation ---
        ("C-q", "quit"),

        // --- help ---
        ("C-h", "help"),
        ("M-h:k", "help-keys"),
        ("M-h:o", "help-ops"),
        ("M-h:b", "help-bindings"),
        ("M-h:c", "help-colors"),

        // --- navigation and selection ---
        ("C-b", "move-backward"),
        ("left", "move-backward"),
        ("S-left", "move-backward-select"),
        ("C-f", "move-forward"),
        ("right", "move-forward"),
        ("S-right", "move-forward-select"),
        ("C-p", "move-up"),
        ("up", "move-up"),
        ("S-up", "move-up-select"),
        ("C-n", "move-down"),
        ("down", "move-down"),
        ("S-down", "move-down-select"),
        ("M-p", "move-up-page"),
        ("pg_up", "move-up-page"),
        ("S-pg_up", "move-up-page-select"),
        ("M-n", "move-down-page"),
        ("pg_down", "move-down-page"),
        ("S-pg_down", "move-down-page-select"),
        ("C-a", "move-start"),
        ("home", "move-start"),
        ("S-home", "move-start-select"),
        ("C-e", "move-end"),
        ("end", "move-end"),
        ("S-end", "move-end-select"),
        ("C-home", "move-top"),
        ("M-a", "move-top"),
        ("S-C-home", "move-top-select"),
        ("C-end", "move-bottom"),
        ("M-e", "move-bottom"),
        ("S-C-end", "move-bottom-select"),
        ("M-b", "move-backward-word"),
        ("C-left", "move-backward-word"),
        ("M-B", "move-backward-word-select"),
        ("S-C-left", "move-backward-word-select"),
        ("M-f", "move-forward-word"),
        ("C-right", "move-forward-word"),
        ("M-F", "move-forward-word-select"),
        ("S-C-right", "move-forward-word-select"),
        ("C-up", "scroll-up"),
        ("S-C-up", "scroll-up-select"),
        ("C-down", "scroll-down"),
        ("S-C-down", "scroll-down-select"),
        ("C-l", "scroll-center"),
        ("M-l", "redraw"),
        ("C-@", "set-mark"),
        ("C-_", "goto-line"),

        // --- insertion and removal ---
        ("ret", "insert-line"),
        ("tab", "insert-tab"),
        ("C-^", "insert-unicode-dec"),
        ("M-^", "insert-unicode-hex"),
        ("C-d", "remove-after"),
        ("del", "remove-before"),
        ("C-j", "remove-start"),
        ("C-k", "remove-end"),
        ("C-u", "undo"),
        ("C-r", "redo"),

        // --- selection actions ---
        ("C-c", "copy"),
        ("M-c", "copy-global"),
        ("C-v", "paste"),
        ("M-v", "paste-global"),
        ("C-x", "cut"),
        ("M-x", "cut-global"),

        // --- search ---
        ("C-\\", "search"),
        ("M-C-\\", "search-case"),
        ("M-\\", "search-regex"),
        ("M-M-\\", "search-regex-case"),
        ("C-]", "search-next"),

        // --- file handling ---
        ("C-o", "open-file"),
        ("M-o:a", "open-file-top"),
        ("M-o:e", "open-file-bottom"),
        ("M-o:p", "open-file-above"),
        ("M-o:n", "open-file-below"),
        ("C-s", "save-file"),
        ("M-s", "save-file-as"),

        // --- editor handling ---
        ("C-y", "select-editor"),
        ("M-y:a", "select-editor-top"),
        ("M-y:e", "select-editor-bottom"),
        ("M-y:p", "select-editor-above"),
        ("M-y:n", "select-editor-below"),
        ("M-,", "prev-editor"),
        ("M-.", "next-editor"),

        // --- window handling ---
        ("C-w", "kill-window"),
        ("M-w:0", "close-window"),
        ("M-w:1", "close-other-windows"),
        ("M-w:a", "top-window"),
        ("M-w:e", "bottom-window"),
        ("M-w:p", "prev-window"),
        ("M-<", "prev-window"),
        ("M-w:n", "next-window"),
        ("M->", "next-window"),

        // --- behaviors ---
        ("C-t", "describe-editor"),
        ("M-t:t", "tab-mode"),
    ];
}

impl Default for Configuration {
    fn default() -> Configuration {
        Configuration {
            settings: Settings::default(),
            colors: Colors::default(),
            theme: Theme::default(),
            bindings: Self::init_bindings(),
            registry: Registry::default(),
        }
    }
}
