use std::env;
use std::fs;
use std::io::{Error as IoError, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use ansi_term::{Color, Style};
use app_dirs::{get_app_root, AppDataType};
use log::debug;
use serde_derive::{Deserialize, Serialize};

use crate::error::TealdeerError::{self, ConfigError};
use crate::types::PathSource;

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const MAX_CACHE_AGE: Duration = Duration::from_secs(2_592_000); // 30 days
const DEFAULT_UPDATE_INTERVAL_HOURS: u64 = MAX_CACHE_AGE.as_secs() / 3600; // 30 days

fn default_underline() -> bool {
    false
}

fn default_bold() -> bool {
    false
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum RawColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Purple,
    Cyan,
    White,
    Ansi(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

impl From<RawColor> for Color {
    fn from(raw_color: RawColor) -> Self {
        match raw_color {
            RawColor::Black => Self::Black,
            RawColor::Red => Self::Red,
            RawColor::Green => Self::Green,
            RawColor::Yellow => Self::Yellow,
            RawColor::Blue => Self::Blue,
            RawColor::Purple => Self::Purple,
            RawColor::Cyan => Self::Cyan,
            RawColor::White => Self::White,
            RawColor::Ansi(num) => Self::Fixed(num),
            RawColor::Rgb { r, g, b } => Self::RGB(r, g, b),
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RawStyle {
    pub foreground: Option<RawColor>,
    pub background: Option<RawColor>,
    #[serde(default = "default_underline")]
    pub underline: bool,
    #[serde(default = "default_bold")]
    pub bold: bool,
}

impl Default for RawStyle {
    fn default() -> Self {
        Self {
            foreground: None,
            background: None,
            underline: false,
            bold: false,
        }
    }
} // impl RawStyle

impl From<RawStyle> for Style {
    fn from(raw_style: RawStyle) -> Self {
        let mut style = Self::default();

        if let Some(foreground) = raw_style.foreground {
            style = style.fg(Color::from(foreground));
        }

        if let Some(background) = raw_style.background {
            style = style.on(Color::from(background));
        }

        if raw_style.underline {
            style = style.underline();
        }

        if raw_style.bold {
            style = style.bold();
        }

        style
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RawStyleConfig {
    #[serde(default)]
    pub description: RawStyle,
    #[serde(default)]
    pub command_name: RawStyle,
    #[serde(default)]
    pub example_text: RawStyle,
    #[serde(default)]
    pub example_code: RawStyle,
    #[serde(default)]
    pub example_variable: RawStyle,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RawDisplayConfig {
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub use_pager: bool,
}

/// Serde doesn't support default values yet (tracking issue:
/// <https://github.com/serde-rs/serde/issues/368>), so we need to wrap
/// `DEFAULT_UPDATE_INTERVAL_HOURS` in a function to be able to use
/// `#[serde(default = ...)]`
const fn default_auto_update_interval_hours() -> u64 {
    DEFAULT_UPDATE_INTERVAL_HOURS
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RawUpdatesConfig {
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default = "default_auto_update_interval_hours")]
    pub auto_update_interval_hours: u64,
}

impl Default for RawUpdatesConfig {
    fn default() -> Self {
        Self {
            auto_update: false,
            auto_update_interval_hours: DEFAULT_UPDATE_INTERVAL_HOURS,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RawDirectoriesConfig {
    #[serde(default)]
    pub custom_pages_dir: PathBuf,
}

impl Default for RawDirectoriesConfig {
    fn default() -> Self {
        Self {
            custom_pages_dir: get_app_root(AppDataType::UserData, &crate::APP_INFO).map_or_else(
                |_| {
                    eprintln!("Could not locate the App Root");
                    std::process::exit(1);
                },
                |p| p.join("pages"),
            ),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RawConfig {
    #[serde(default)]
    style: RawStyleConfig,
    #[serde(default)]
    display: RawDisplayConfig,
    #[serde(default)]
    updates: RawUpdatesConfig,
    #[serde(default)]
    directories: RawDirectoriesConfig,
}

impl RawConfig {
    fn new() -> Self {
        let mut raw_config = Self::default();

        // Set default config
        raw_config.style.example_text.foreground = Some(RawColor::Green);
        raw_config.style.command_name.foreground = Some(RawColor::Cyan);
        raw_config.style.example_code.foreground = Some(RawColor::Cyan);
        raw_config.style.example_variable.foreground = Some(RawColor::Cyan);
        raw_config.style.example_variable.underline = true;

        raw_config
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StyleConfig {
    pub description: Style,
    pub command_name: Style,
    pub example_text: Style,
    pub example_code: Style,
    pub example_variable: Style,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DisplayConfig {
    pub compact: bool,
    pub use_pager: bool,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UpdatesConfig {
    pub auto_update: bool,
    pub auto_update_interval: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectoriesConfig {
    pub custom_pages_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub style: StyleConfig,
    pub display: DisplayConfig,
    pub updates: UpdatesConfig,
    pub directories: DirectoriesConfig,
}

impl From<RawConfig> for Config {
    fn from(raw_config: RawConfig) -> Self {
        Self {
            style: StyleConfig {
                command_name: raw_config.style.command_name.into(),
                description: raw_config.style.description.into(),
                example_text: raw_config.style.example_text.into(),
                example_code: raw_config.style.example_code.into(),
                example_variable: raw_config.style.example_variable.into(),
            },
            display: DisplayConfig {
                compact: raw_config.display.compact,
                use_pager: raw_config.display.use_pager,
            },
            updates: UpdatesConfig {
                auto_update: raw_config.updates.auto_update,
                auto_update_interval: Duration::from_secs(
                    raw_config.updates.auto_update_interval_hours * 3600,
                ),
            },
            directories: DirectoriesConfig {
                custom_pages_dir: raw_config.directories.custom_pages_dir,
            },
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn map_io_err_to_config_err(e: IoError) -> TealdeerError {
    ConfigError(format!("Io Error: {}", e))
}

impl Config {
    pub fn load(enable_styles: bool) -> Result<Self, TealdeerError> {
        debug!("Loading config");

        // Determine path
        let (config_file_path, _) = get_config_path()
            .map_err(|e| ConfigError(format!("Could not determine config path: {}", e)))?;

        // Load raw config
        let raw_config: RawConfig = if config_file_path.exists() && config_file_path.is_file() {
            let mut config_file =
                fs::File::open(config_file_path).map_err(map_io_err_to_config_err)?;
            let mut contents = String::new();
            let _ = config_file
                .read_to_string(&mut contents)
                .map_err(map_io_err_to_config_err)?;
            toml::from_str(&contents)
                .map_err(|err| ConfigError(format!("Failed to parse config file: {}", err)))?
        } else {
            RawConfig::new()
        };

        // Convert to config
        let mut config = Self::from(raw_config);

        // Potentially override styles
        if !enable_styles {
            config.style = StyleConfig {
                command_name: Style::default(),
                description: Style::default(),
                example_text: Style::default(),
                example_code: Style::default(),
                example_variable: Style::default(),
            };
        }

        Ok(config)
    }
}

/// Return the path to the config directory.
///
/// The config dir path can be overridden using the `TEALDEER_CONFIG_DIR` env
/// variable. Otherwise, the user config directory is returned.
///
/// Note that this function does not verify whether the directory at that
/// location exists, or is a directory.
pub fn get_config_dir() -> Result<(PathBuf, PathSource), TealdeerError> {
    // Allow overriding the config directory by setting the
    // $TEALDEER_CONFIG_DIR env variable.
    if let Ok(value) = env::var("TEALDEER_CONFIG_DIR") {
        return Ok((PathBuf::from(value), PathSource::EnvVar));
    };

    // Otherwise, fall back to the user config directory.
    match get_app_root(AppDataType::UserConfig, &crate::APP_INFO) {
        Ok(dirs) => Ok((dirs, PathSource::OsConvention)),
        Err(_) => Err(ConfigError(
            "Could not determine the user config directory.".into(),
        )),
    }
}

/// Return the path to the config file.
///
/// Note that this function does not verify whether the file at that location
/// exists, or is a file.
pub fn get_config_path() -> Result<(PathBuf, PathSource), TealdeerError> {
    let (config_dir, source) = get_config_dir()?;
    let config_file_path = config_dir.join(CONFIG_FILE_NAME);
    Ok((config_file_path, source))
}

/// Create default config file.
pub fn make_default_config() -> Result<PathBuf, TealdeerError> {
    let (config_dir, _) = get_config_dir()?;

    // Ensure that config directory exists
    if !config_dir.exists() {
        if let Err(e) = fs::create_dir_all(&config_dir) {
            return Err(ConfigError(format!(
                "Could not create config directory: {}",
                e
            )));
        }
    } else if !config_dir.is_dir() {
        return Err(ConfigError(format!(
            "Config directory could not be created: {} already exists but is not a directory",
            config_dir.to_string_lossy(),
        )));
    }

    // Ensure that a config file doesn't get overwritten
    let config_file_path = config_dir.join(CONFIG_FILE_NAME);
    if config_file_path.is_file() {
        return Err(ConfigError(format!(
            "A configuration file already exists at {}, no action was taken.",
            config_file_path.to_str().unwrap()
        )));
    }

    // Create default config
    let serialized_config = toml::to_string(&RawConfig::new())
        .map_err(|err| ConfigError(format!("Failed to serialize default config: {}", err)))?;

    // Write default config
    let mut config_file = fs::File::create(&config_file_path).map_err(map_io_err_to_config_err)?;
    let _wc = config_file
        .write(serialized_config.as_bytes())
        .map_err(map_io_err_to_config_err)?;

    Ok(config_file_path)
}

#[test]
fn test_serialize_deserialize() {
    let raw_config = RawConfig::new();
    let serialized = toml::to_string(&raw_config).unwrap();
    let deserialized: RawConfig = toml::from_str(&serialized).unwrap();
    assert_eq!(raw_config, deserialized);
}
