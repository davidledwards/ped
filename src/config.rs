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
//! * `$HOME/.config/ped/pedrc`

use crate::bind::Bindings;
use crate::color::Color;
use crate::error::{Error, Result};
use crate::opt::Options;
use crate::sys::{self, AsString};
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::result;

/// A configuration representing all aspects of the editing experience.
pub struct Configuration {
    /// A collection of configurable settings that control the behavior and rendering
    /// of editors.
    pub settings: Settings,

    /// A collection of configurable colors.
    pub colors: Colors,

    /// A map of key sequences to editing operations.
    pub bindings: Bindings,
}

pub type ConfigurationRef = Rc<Configuration>;

pub struct Settings {
    pub show_spotlight: bool,
    pub show_lines: bool,
    pub show_eol: bool,
    pub tab_size: usize,
}

pub struct Colors {
    pub text: Color,
    pub select: Color,
    pub banner: Color,
    pub echo: Color,
    pub prompt: Color,
    pub spotlight: Color,
    pub line: Color,
    pub eol: Color,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalConfiguration {
    settings: Option<ExternalSettings>,
    colors: Option<ExternalColors>,
    bindings: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalSettings {
    #[serde(rename = "show-spotlight")]
    show_spotlight: Option<bool>,

    #[serde(rename = "show-lines")]
    show_lines: Option<bool>,

    #[serde(rename = "show-eol")]
    show_eol: Option<bool>,

    #[serde(rename = "tab-size")]
    tab_size: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalColors {
    text: Option<Color>,
    select: Option<Color>,
    banner: Option<Color>,
    echo: Option<Color>,
    prompt: Option<Color>,
    spotlight: Option<Color>,
    line: Option<Color>,
    eol: Option<Color>,
}

impl Settings {
    /// Applies the external settings `ext` on top of `self`.
    fn apply(&mut self, ext: Option<ExternalSettings>) {
        if let Some(ext) = ext {
            self.show_spotlight = ext.show_spotlight.unwrap_or(self.show_spotlight);
            self.show_lines = ext.show_lines.unwrap_or(self.show_lines);
            self.show_eol = ext.show_eol.unwrap_or(self.show_eol);
            self.tab_size = ext.tab_size.unwrap_or(self.tab_size);
        }
    }

    /// Applies the relevant settings from `opts` on top of `self`.
    pub fn apply_opts(&mut self, opts: &Options) {
        self.show_spotlight = opts.show_spotlight.unwrap_or(self.show_spotlight);
        self.show_lines = opts.show_lines.unwrap_or(self.show_lines);
        self.show_eol = opts.show_eol.unwrap_or(self.show_eol);
        self.tab_size = opts.tab_size.unwrap_or(self.tab_size);
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            show_spotlight: false,
            show_lines: false,
            show_eol: false,
            tab_size: 3,
        }
    }
}

impl Colors {
    /// Applies the external colors `ext` on top of `self`.
    fn apply(&mut self, ext: Option<ExternalColors>) {
        if let Some(ext) = ext {
            self.text = ext.text.unwrap_or(self.text);
            self.select = ext.select.unwrap_or(self.select);
            self.banner = ext.banner.unwrap_or(self.banner);
            self.echo = ext.echo.unwrap_or(self.echo);
            self.prompt = ext.prompt.unwrap_or(self.prompt);
            self.spotlight = ext.spotlight.unwrap_or(self.spotlight);
            self.line = ext.line.unwrap_or(self.line);
            self.eol = ext.eol.unwrap_or(self.eol);
        }
    }
}

impl Default for Colors {
    fn default() -> Colors {
        Colors {
            text: Color::new(252, 232),
            select: Color::new(255, 19),
            banner: Color::new(232, 28),
            echo: Color::new(214, 232),
            prompt: Color::new(34, 232),
            spotlight: Color::new(252, 234),
            line: Color::new(34, 234),
            eol: Color::new(34, 232),
        }
    }
}

impl<'a> Deserialize<'a> for Color {
    fn deserialize<T>(deser: T) -> result::Result<Color, T::Error>
    where
        T: Deserializer<'a>,
    {
        deser.deserialize_tuple(2, ColorVisitor)
    }
}

struct ColorVisitor;

impl<'a> Visitor<'a> for ColorVisitor {
    type Value = Color;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "expecting `[u8, u8]` for Color")
    }

    fn visit_seq<T>(self, mut seq: T) -> result::Result<Color, T::Error>
    where
        T: SeqAccess<'a>,
    {
        let fg = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let bg = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        Ok(Color::new(fg, bg))
    }
}

impl Configuration {
    /// A collection of resource files to try loading in order of precedence.
    const TRY_FILES: [&str; 2] = [".pedrc", ".config/ped/pedrc"];

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
        self.colors.apply(ext.colors);
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

    const DEFAULT_BINDINGS: [(&'static str, &'static str); 83] = [
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
        ("ESC:b", "move-backward-word"),
        ("ESC:left", "move-backward-word"),
        ("S-left", "move-backward-select"),
        ("ESC:B", "move-backward-word-select"),
        ("C-f", "move-forward"),
        ("right", "move-forward"),
        ("ESC:f", "move-forward-word"),
        ("ESC:right", "move-forward-word"),
        ("S-right", "move-forward-select"),
        ("ESC:F", "move-forward-word-select"),
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
        ("S-C-up", "scroll-up"),
        ("S-C-down", "scroll-down"),
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
            bindings: Self::init_bindings(),
        }
    }
}
