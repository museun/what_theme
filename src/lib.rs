use std::path::PathBuf;

use once_cell::sync::Lazy;
use regex::Regex;

/// Found fonts from the configration
#[derive(Debug, PartialEq, Hash)]
pub struct FoundFonts {
    editor: String,
    terminal: String,
}

impl FoundFonts {
    pub fn editor(&self) -> &str {
        self.editor.as_ref()
    }

    pub fn terminal(&self) -> &str {
        self.terminal.as_ref()
    }
}

/// A found theme (extension) from the vscode extension cache
#[derive(Debug, PartialEq, Hash)]
pub struct FoundTheme<'a> {
    id: &'a str,
    variant: &'a str,
}

impl<'a> FoundTheme<'a> {
    /// Gets the url of the extension (theme)
    pub fn url(&self) -> String {
        format!(
            "https://marketplace.visualstudio.com/items?itemName={}",
            self.id
        )
    }

    /// Gets the variant provided during filtering
    pub const fn variant(&self) -> &str {
        self.variant
    }
}

// r#"(?m)^\s*"workbench\.colorTheme"\s*:\s*"(?P<name>.*?)",?\s*?$"#
// r#"(?m)^\s*"editor\.fontFamily"\s*:\s*"(?P<name>.*?)",?\s*?$"#
// r#"(?m)^\s*"terminal\.integrated\.fontFamily"\s*:\s*"(?P<name>.*?)",?\s*?$"#

fn make_json_regex(key: &str) -> Regex {
    let s = key.replace('.', r#"\."#);
    regex::Regex::new(&format!(r#"(?m)^\s*"{}"\s*:\s*"(?P<name>.*?)",?\s*?$"#, s)).unwrap()
}

fn read<F, E>(f: F) -> Result<String>
where
    F: Fn() -> std::result::Result<PathBuf, E>,
    E: Into<Error>,
{
    Ok(std::fs::read_to_string(f().map_err(Into::into)?)?)
}

static WORKBENCH_COLOR_THEME: Lazy<regex::Regex> =
    Lazy::new(|| make_json_regex("workbench.colorTheme"));

static WORKBENCH_EDITOR_FONT: Lazy<regex::Regex> =
    Lazy::new(|| make_json_regex("editor.fontFamily"));

static WORKBENCH_TERMINAL_FONT: Lazy<regex::Regex> =
    Lazy::new(|| make_json_regex("terminal.integrated.fontFamily"));

pub fn settings_json_path() -> Result<PathBuf> {
    Ok(directories::BaseDirs::new()
        .ok_or(Error::CannotFindBaseDir)?
        .config_dir()
        .join("Code")
        .join("User")
        .join("settings.json"))
}

pub fn extension_user_cache_path() -> Result<PathBuf> {
    Ok(directories::BaseDirs::new()
        .ok_or(Error::CannotFindBaseDir)?
        .config_dir()
        .join("Code")
        .join("CachedExtensions")
        .join("user"))
}

/// Reads your current (global) `settings.json` and gets the current active theme
pub fn get_current_theme() -> Result<String> {
    get_current_theme_from(&read(settings_json_path)?)
}

/// Get the current active theme from a `&str`
pub fn get_current_theme_from(data: &str) -> Result<String> {
    Ok(extract(&WORKBENCH_COLOR_THEME, data).ok_or(Error::CannotFindCurrentTheme)?)
}

/// Reads your current (global) `settings.json` and gets the current fonts
pub fn get_current_fonts() -> Result<FoundFonts> {
    get_current_fonts_from(&read(settings_json_path)?)
}

/// Get the current fonts from a `&str`
pub fn get_current_fonts_from(data: &str) -> Result<FoundFonts> {
    let editor = extract(&WORKBENCH_EDITOR_FONT, data).ok_or(Error::CannotFindEditorFont)?;
    let terminal = extract(&WORKBENCH_TERMINAL_FONT, data).ok_or(Error::CannotFindTerminalFont)?;
    Ok(FoundFonts { editor, terminal })
}

fn extract(re: &Regex, data: &str) -> Option<String> {
    re.captures(data).map(|cap| cap["name"].to_string())
}

/// This reads the vscode extension cache and allows you to find/search for installed themes
pub struct VsCodeSettings {
    result: vscode_data::Results,
}

impl VsCodeSettings {
    /// Create a new instance of the `VscodeSettings`
    pub fn new() -> Result<Self> {
        Self::new_from(&read(extension_user_cache_path)?)
    }

    /// Create a new instance of the `VscodeSettings` from str
    pub fn new_from(data: &str) -> Result<Self> {
        let result = serde_json::from_str(data)?;
        Ok(Self { result })
    }

    /// Filters the cache by a variant name
    pub fn find_theme<'a>(&'a self, current: &'a str) -> Option<FoundTheme<'a>> {
        for result in &self.result.result {
            if !result.is_a_theme() || !result.contains_theme(current) {
                continue;
            }
            return Some(FoundTheme {
                id: &*result.identifier.id,
                variant: current,
            });
        }
        None
    }
}

type Result<T> = ::std::result::Result<T, Error>;

/// Errors returned by this crate
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An `XDG_BASE_DIR` could not be found on this system
    #[error("cannot find XDG_BASE_DIR")]
    CannotFindBaseDir,

    /// An `I/O` error occurred
    #[error("an i/o error happened")]
    Io(#[from] std::io::Error),

    /// Cannot parse/find the current theme
    #[error("cannot find current theme")]
    CannotFindCurrentTheme,

    /// Cannot parse/find the current editor font
    #[error("cannot find current editor font")]
    CannotFindEditorFont,

    /// Cannot parse/find the current terminal font
    #[error("cannot find current terminal font")]
    CannotFindTerminalFont,

    /// A serialization problem
    #[error("cannot deserialize user cache file")]
    Json(#[from] serde_json::Error),
}

mod vscode_data {
    #[derive(::serde::Deserialize)]
    pub(crate) struct Results {
        pub(crate) result: Vec<Result>,
    }

    #[derive(::serde::Deserialize)]
    pub(crate) struct Result {
        pub(crate) identifier: Identifier,
        pub(crate) manifest: Manifest,
    }

    impl Result {
        pub(crate) fn is_a_theme(&self) -> bool {
            self.manifest.categories.iter().any(|c| c == "Themes")
        }

        pub(crate) fn contains_theme(&self, theme: &str) -> bool {
            self.manifest
                .contributes
                .themes
                .iter()
                .any(|c| c.label == theme)
        }
    }

    #[derive(::serde::Deserialize, Default)]
    #[serde(default)]
    pub(crate) struct Identifier {
        pub(crate) id: String,
    }

    #[derive(::serde::Deserialize, Default)]
    #[serde(default)]
    pub(crate) struct Manifest {
        categories: Vec<String>,
        contributes: Contributes,
    }

    #[derive(::serde::Deserialize, Default)]
    pub(crate) struct Contributes {
        #[serde(default)]
        themes: Vec<Theme>,
    }

    #[derive(::serde::Deserialize)]
    pub(crate) struct Theme {
        label: String,
    }
}
