use std::path::PathBuf;

use once_cell::sync::Lazy;
use regex_lite::Regex;

/// Found fonts from the configration
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

fn make_json_regex(key: &str) -> Regex {
    let s = key.replace('.', r#"\."#);
    regex_lite::Regex::new(&format!(r#"(?m)^\s*"{}"\s*:\s*"(?P<name>.*?)",?\s*?$"#, s)).unwrap()
}

fn read<F, E>(f: F) -> Result<String>
where
    F: Fn() -> std::result::Result<PathBuf, E>,
    E: Into<Error>,
{
    Ok(std::fs::read_to_string(f().map_err(Into::into)?)?)
}

static WORKBENCH_COLOR_THEME: Lazy<regex_lite::Regex> =
    Lazy::new(|| make_json_regex("workbench.colorTheme"));

static WORKBENCH_EDITOR_FONT: Lazy<regex_lite::Regex> =
    Lazy::new(|| make_json_regex("editor.fontFamily"));

static WORKBENCH_TERMINAL_FONT: Lazy<regex_lite::Regex> =
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
    Ok(directories::UserDirs::new()
        .ok_or(Error::CannotFindBaseDir)?
        .home_dir()
        .join(".vscode")
        .join("extensions")
        .join("extensions.json"))
}

/// Reads your current (global) `settings.json` and gets the current active theme
pub fn get_current_theme() -> Result<String> {
    get_current_theme_from(&read(settings_json_path)?)
}

/// Get the current active theme from a `&str`
pub fn get_current_theme_from(data: &str) -> Result<String> {
    extract(&WORKBENCH_COLOR_THEME, data).ok_or(Error::CannotFindCurrentTheme)
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
    list: Vec<LabeledTheme>,
}

impl VsCodeSettings {
    /// Create a new instance of the `VscodeSettings`
    pub fn new() -> Result<Self> {
        Self::new_from(&read(extension_user_cache_path)?)
    }

    /// Create a new instance of the `VscodeSettings` from str
    pub fn new_from(data: &str) -> Result<Self> {
        let list = Self::collate_themes(serde_json::from_str(data)?);
        Ok(Self { list })
    }

    /// Filters the cache by a variant name
    pub fn find_theme<'a>(&'a self, current: &'a str) -> Option<FoundTheme<'a>> {
        self.list.iter().find_map(|c| {
            if &*c.label == current {
                Some(FoundTheme {
                    id: &c.id,
                    variant: &c.label,
                })
            } else {
                None
            }
        })
    }

    fn collate_themes(list: Vec<vscode_data::Result>) -> Vec<LabeledTheme> {
        fn undo_the_node_path_thing(s: &str) -> &str {
            if cfg!(target_os = "windows") {
                return s.strip_prefix('/').unwrap_or(s);
            }
            s
        }

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            for result in list {
                scope.spawn({
                    let tx = tx.clone();
                    move || {
                        let path = undo_the_node_path_thing(&result.location.path);
                        let path = match PathBuf::from(path).join("package.json").canonicalize() {
                            Ok(path) => path,
                            Err(_err) => return,
                        };

                        if let Ok(data) = std::fs::read_to_string(&path) {
                            if let Ok(manifest) =
                                serde_json::from_str::<vscode_data::Manifest>(&data)
                            {
                                for theme in manifest.contributes.themes {
                                    let _ = tx.send(LabeledTheme {
                                        label: theme.label,
                                        id: result.identifier.id.clone(),
                                    });
                                }
                            }
                        }
                    }
                });
            }
        });
        drop(tx);

        rx.into_iter().collect()
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

#[derive(Debug)]
pub struct LabeledTheme {
    pub label: String,
    pub id: String,
}

mod vscode_data {
    #[derive(::serde::Deserialize, Debug)]
    pub struct Result {
        pub identifier: Identifier,
        pub location: Location,
    }

    #[derive(::serde::Deserialize, Debug)]
    pub struct Location {
        pub path: String,
    }

    #[derive(::serde::Deserialize, Default, Debug, Clone)]
    #[serde(default)]
    pub struct Identifier {
        pub id: String,
    }

    #[derive(::serde::Deserialize, Default, Debug)]
    #[serde(default)]
    pub struct Manifest {
        pub categories: Vec<String>,
        pub contributes: Contributes,
    }

    #[derive(::serde::Deserialize, Default, Debug)]
    pub struct Contributes {
        #[serde(default)]
        pub themes: Vec<Theme>,
    }

    #[derive(::serde::Deserialize, Debug)]
    pub struct Theme {
        pub label: String,
    }
}

// impl Result {
//     // pub fn is_a_theme(&self) -> bool {
//     //     self.manifest.categories.iter().any(|c| c == "Themes")
//     // }

//     // pub fn contains_theme(&self, theme: &str) -> bool {
//     //     self.manifest
//     //         .contributes
//     //         .themes
//     //         .iter()
//     //         .any(|c| c.label == theme)
//     // }
// }
