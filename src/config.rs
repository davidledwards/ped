//! Contains everything related to configuration.
//!
//! All default values for configurable aspects of the editor are defined in this
//! module, including but not necessarily exclusive to *settings*, *colors*, and
//! *key bindings*.
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
use crate::color::{Color, ColorValue, Colors};
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
    pub tab_size: usize,
}

pub struct Theme {
    pub text_fg: u8,
    pub text_bg: u8,
    pub select_bg: u8,
    pub spotlight_bg: u8,
    pub whitespace_fg: u8,
    pub echo_fg: u8,
    pub prompt_fg: u8,
    pub banner_fg: u8,
    pub banner_bg: u8,
    pub margin_fg: u8,
    pub margin_bg: u8,
    pub text_color: Color,
    pub echo_color: Color,
    pub prompt_color: Color,
    pub banner_color: Color,
    pub margin_color: Color,
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
    tab_size: Option<usize>,
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

    #[serde(rename = "echo-fg")]
    echo_fg: Option<ColorValue>,

    #[serde(rename = "prompt-fg")]
    prompt_fg: Option<ColorValue>,

    #[serde(rename = "banner-fg")]
    banner_fg: Option<ColorValue>,

    #[serde(rename = "banner-bg")]
    banner_bg: Option<ColorValue>,

    #[serde(rename = "margin-fg")]
    margin_fg: Option<ColorValue>,

    #[serde(rename = "margin-bg")]
    margin_bg: Option<ColorValue>,
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
        }
    }

    /// Applies the relevant settings from `opts` on top of `self`.
    pub fn apply_opts(&mut self, opts: &Options) {
        self.spotlight = opts.spotlight.unwrap_or(self.spotlight);
        self.lines = opts.lines.unwrap_or(self.lines);
        self.eol = opts.eol.unwrap_or(self.eol);
        self.tab_hard = opts.tab_hard.unwrap_or(self.tab_hard);
        self.tab_size = opts.tab_size.unwrap_or(self.tab_size);
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            spotlight: false,
            lines: false,
            eol: false,
            tab_hard: false,
            tab_size: 4,
        }
    }
}

impl Theme {
    const TEXT_FG: u8 = 252;
    const TEXT_BG: u8 = 232;
    const SELECT_BG: u8 = 19;
    const SPOTLIGHT_BG: u8 = 234;
    const WHITSPACE_FG: u8 = 243;
    const ECHO_FG: u8 = 214;
    const PROMPT_FG: u8 = 34;
    const BANNER_FG: u8 = 255;
    const BANNER_BG: u8 = 22;
    const MARGIN_FG: u8 = 34;
    const MARGIN_BG: u8 = 234;

    /// Applies the external theme `ext` on top of `self`.
    fn apply(&mut self, ext: Option<ExternalTheme>, colors: &Colors) -> Result<()> {
        fn resolve(color: u8, try_color: &Option<ColorValue>, colors: &Colors) -> Result<u8> {
            if let Some(try_color) = try_color {
                if let Some(color) = colors.lookup_value(&try_color) {
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
            self.echo_fg = resolve(self.echo_fg, &ext.echo_fg, colors)?;
            self.prompt_fg = resolve(self.prompt_fg, &ext.prompt_fg, colors)?;
            self.banner_fg = resolve(self.banner_fg, &ext.banner_fg, colors)?;
            self.banner_bg = resolve(self.banner_bg, &ext.banner_bg, colors)?;
            self.margin_fg = resolve(self.margin_fg, &ext.margin_fg, colors)?;
            self.margin_bg = resolve(self.margin_bg, &ext.margin_bg, colors)?;

            // These are preconstructed colors combining fg/bg primarily as convenience.
            self.text_color = Color::new(self.text_fg, self.text_bg);
            self.echo_color.fg = self.echo_fg;
            self.prompt_color.fg = self.prompt_fg;
            self.banner_color = Color::new(self.banner_fg, self.banner_bg);
            self.margin_color = Color::new(self.margin_fg, self.margin_bg);
        }
        Ok(())
    }
}

impl Default for Theme {
    fn default() -> Theme {
        let text_color = Color::new(Self::TEXT_FG, Self::TEXT_BG);
        let echo_color = Color::new(Self::ECHO_FG, text_color.bg);
        let prompt_color = Color::new(Self::PROMPT_FG, text_color.bg);
        let banner_color = Color::new(Self::BANNER_FG, Self::BANNER_BG);
        let margin_color = Color::new(Self::MARGIN_FG, Self::MARGIN_BG);

        Theme {
            text_fg: text_color.fg,
            text_bg: text_color.bg,
            select_bg: Self::SELECT_BG,
            spotlight_bg: Self::SPOTLIGHT_BG,
            whitespace_fg: Self::WHITSPACE_FG,
            echo_fg: echo_color.fg,
            prompt_fg: prompt_color.fg,
            banner_fg: banner_color.fg,
            banner_bg: banner_color.bg,
            margin_fg: margin_color.fg,
            margin_bg: margin_color.bg,
            text_color,
            echo_color,
            prompt_color,
            banner_color,
            margin_color,
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
    pub fn to_ref(self) -> ConfigurationRef {
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

    const DEFAULT_BINDINGS: [(&'static str, &'static str); 87] = [
        // --- exit and cancellation ---
        ("C-q", "quit"),
        // --- help ---
        ("C-h", "help"),
        ("ESC:h:k", "help-keys"),
        ("ESC:h:o", "help-ops"),
        ("ESC:h:b", "help-bindings"),
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
        ("ESC:p", "move-up-page"),
        ("pg_up", "move-up-page"),
        ("S-pg_up", "move-up-page-select"),
        ("ESC:n", "move-down-page"),
        ("pg_down", "move-down-page"),
        ("S-pg_down", "move-down-page-select"),
        ("C-a", "move-start"),
        ("home", "move-start"),
        ("S-home", "move-start-select"),
        ("C-e", "move-end"),
        ("end", "move-end"),
        ("S-end", "move-end-select"),
        ("C-home", "move-top"),
        ("ESC:a", "move-top"),
        ("S-C-home", "move-top-select"),
        ("C-end", "move-bottom"),
        ("ESC:e", "move-bottom"),
        ("S-C-end", "move-bottom-select"),
        ("ESC:b", "move-backward-word"),
        ("C-left", "move-backward-word"),
        ("ESC:B", "move-backward-word-select"),
        ("S-C-left", "move-backward-word-select"),
        ("ESC:f", "move-forward-word"),
        ("C-right", "move-forward-word"),
        ("ESC:F", "move-forward-word-select"),
        ("S-C-right", "move-forward-word-select"),
        ("C-up", "scroll-up"),
        ("S-C-up", "scroll-up-select"),
        ("C-down", "scroll-down"),
        ("S-C-down", "scroll-down-select"),
        ("C-l", "scroll-center"),
        ("C-@", "set-mark"),
        ("C-_", "goto-line"),
        // --- insertion and removal ---
        ("ret", "insert-line"),
        ("tab", "insert-tab"),
        ("C-d", "remove-after"),
        ("del", "remove-before"),
        ("C-j", "remove-start"),
        ("C-k", "remove-end"),
        ("C-u", "undo"),
        ("C-r", "redo"),
        // --- selection actions ---
        ("C-c", "copy"),
        ("C-v", "paste"),
        ("C-x", "cut"),
        // --- search next ---
        ("C-\\", "search"),
        ("ESC:\\", "search-regex"),
        ("C-]", "search-next"),
        // --- file handling ---
        ("C-o", "open-file"),
        ("ESC:o:t", "open-file-top"),
        ("ESC:o:b", "open-file-bottom"),
        ("ESC:o:p", "open-file-above"),
        ("ESC:o:n", "open-file-below"),
        ("C-s", "save-file"),
        ("ESC:s", "save-file-as"),
        // --- editor handling ---
        ("C-y", "select-editor"),
        ("ESC:y:t", "select-editor-top"),
        ("ESC:y:b", "select-editor-bottom"),
        ("ESC:y:p", "select-editor-above"),
        ("ESC:y:n", "select-editor-below"),
        ("ESC:,", "prev-editor"),
        ("ESC:.", "next-editor"),
        // --- window handling ---
        ("C-w", "kill-window"),
        ("ESC:w:0", "close-window"),
        ("ESC:w:1", "close-other-windows"),
        ("ESC:w:t", "top-window"),
        ("ESC:w:b", "bottom-window"),
        ("ESC:w:p", "prev-window"),
        ("ESC:<", "prev-window"),
        ("ESC:w:n", "next-window"),
        ("ESC:>", "next-window"),
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
