use std::path::PathBuf;

use once_cell::sync::Lazy;

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

impl<'a> std::fmt::Display for FoundTheme<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' from {}", self.variant(), self.url())
    }
}

const WORKBENCH_COLOR_THEME: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(r#"(?m)^\s*"workbench\.colorTheme"\s*:\s*"(?P<name>.*?)",?\s*?$"#).unwrap()
});

/// Reads your current (global) `settings.json` and gets the current active theme
pub fn get_current_theme() -> Result<String> {
    let data = VsCodeSettings::read_data(|f| f.join("User").join("settings.json"))?;
    get_current_theme_from(&data)
}

/// Get the current active theme from a `&str`
pub fn get_current_theme_from(data: &str) -> Result<String> {
    WORKBENCH_COLOR_THEME
        .captures(data)
        .ok_or_else(|| Error::CannotFindCurrentTheme)
        .map(|cap| cap["name"].to_string())
}

/// This reads the vscode extension cache and allows you to find/search for installed themes
pub struct VsCodeSettings {
    result: vscode_data::Results,
}

impl VsCodeSettings {
    /// Create a new instance of the `VscodeSettings`
    pub fn new() -> Result<Self> {
        let json = Self::read_data(|f| f.join("CachedExtensions").join("user"))?;
        Self::new_from(&json)
    }

    /// Create a new instance of the `VscodeSettings` from str
    pub fn new_from(data: &str) -> Result<Self> {
        Ok(Self {
            result: serde_json::from_str(data)?,
        })
    }

    pub fn settings_json_path() -> Result<PathBuf> {
        Ok(directories::BaseDirs::new()
            .ok_or_else(|| Error::CannotFindBaseDir)?
            .config_dir()
            .join("Code")
            .join("User")
            .join("settings.json"))
    }

    pub fn extension_user_cache_path() -> Result<PathBuf> {
        Ok(directories::BaseDirs::new()
            .ok_or_else(|| Error::CannotFindBaseDir)?
            .config_dir()
            .join("Code")
            .join("CachedExtensions")
            .join("user"))
    }

    /// Filters the cache by a variant name
    pub fn find_theme<'a>(&'a self, current: &'a str) -> Option<FoundTheme<'a>> {
        for result in &self.result.result {
            let manifest = &result.manifest;

            if !manifest.is_a_theme() || !manifest.contains_theme(current) {
                continue;
            }

            return Some(FoundTheme {
                id: &*result.identifier.id,
                variant: current,
            });
        }

        None
    }

    fn read_data(f: fn(PathBuf) -> PathBuf) -> Result<String> {
        let path = f(directories::BaseDirs::new()
            .ok_or_else(|| Error::CannotFindBaseDir)?
            .config_dir()
            .join("Code"));
        Ok(std::fs::read_to_string(path)?)
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

    impl Manifest {
        pub(crate) fn is_a_theme(&self) -> bool {
            self.categories.iter().any(|c| c == "Themes")
        }

        pub(crate) fn contains_theme(&self, theme: &str) -> bool {
            self.contributes.themes.iter().any(|c| c.label == theme)
        }
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
