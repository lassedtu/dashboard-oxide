use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Written to ~/.dbx/config.toml on first run. Must stay in sync with the
/// `Default` impls below (enforced by the `template_matches_defaults` test).
const DEFAULT_CONFIG: &str = r#"# dbx configuration

# Folder where new projects are created and recent projects are listed from.
projects_dir = "~/Developer/Projekter"

# Command used to open a project folder. May include arguments, e.g. "code -n".
# Terminal editors (nvim, vim, hx, ...) take over the terminal while open.
editor = "code"

# How many projects to show under "Recent projects".
recent_count = 10

# Quit dbx after opening a project in the editor.
close_on_open = true

[new_project]
git_init = true
create_readme = true
"#;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default)] // missing keys fall back to Default, so old configs keep working
pub struct Config {
    /// Where this config was loaded from (not a setting).
    #[serde(skip)]
    pub path: PathBuf,
    pub projects_dir: PathBuf,
    pub editor: String,
    pub recent_count: usize,
    pub close_on_open: bool,
    pub new_project: NewProjectConfig,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct NewProjectConfig {
    pub git_init: bool,
    pub create_readme: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            projects_dir: PathBuf::from("~/Developer/Projekter"),
            editor: "code".into(),
            recent_count: 10,
            close_on_open: true,
            new_project: NewProjectConfig::default(),
        }
    }
}

impl Default for NewProjectConfig {
    fn default() -> Self {
        Self { git_init: true, create_readme: true }
    }
}

impl Config {
    /// Load ~/.dbx/config.toml, creating it with defaults if it doesn't exist.
    pub fn load() -> Result<Self> {
        Self::load_from(&config_dir()?.join("config.toml"))
    }

    /// Re-read the file this config came from, picking up external edits.
    pub fn reload(&self) -> Result<Self> {
        Self::load_from(&self.path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            fs::write(path, DEFAULT_CONFIG)
                .with_context(|| format!("writing {}", path.display()))?;
        }

        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut config: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;

        config.projects_dir = expand_tilde(&config.projects_dir)?;
        config.path = path.to_path_buf();
        Ok(config)
    }
}

/// Set one key in the config file at `path`, keeping comments and formatting.
/// `key` may be dotted for tables, e.g. `"new_project.git_init"`.
pub fn set(path: &Path, key: &str, value: impl Into<toml_edit::Value>) -> Result<()> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut doc: toml_edit::DocumentMut =
        text.parse().with_context(|| format!("parsing {}", path.display()))?;

    let mut parts = key.split('.');
    let mut item = &mut doc[parts.next().context("empty config key")?];
    for part in parts {
        item = &mut item[part];
    }
    *item = toml_edit::value(value);

    fs::write(path, doc.to_string()).with_context(|| format!("writing {}", path.display()))
}

/// Show a path with the home directory abbreviated to `~`.
pub fn tilde(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// ~/.dbx — the home for config now, and templates/state later.
pub fn config_dir() -> Result<PathBuf> {
    Ok(home()?.join(".dbx"))
}

fn home() -> Result<PathBuf> {
    dirs::home_dir().context("could not determine home directory")
}

pub fn expand_tilde(path: &Path) -> Result<PathBuf> {
    match path.strip_prefix("~") {
        Ok(rest) => Ok(home()?.join(rest)),
        Err(_) => Ok(path.to_path_buf()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_matches_defaults() {
        let parsed: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn missing_keys_use_defaults() {
        let parsed: Config = toml::from_str("recent_count = 3").unwrap();
        assert_eq!(parsed.recent_count, 3);
        assert_eq!(parsed.editor, "code");
        assert!(parsed.new_project.git_init);
    }

    #[test]
    fn expands_tilde() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde(Path::new("~/foo")).unwrap(), home.join("foo"));
        assert_eq!(expand_tilde(Path::new("/abs")).unwrap(), PathBuf::from("/abs"));
    }

    #[test]
    fn creates_file_on_first_load() {
        let dir = std::env::temp_dir().join(format!("dbx-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = fs::remove_dir_all(&dir);

        let config = Config::load_from(&path).unwrap();
        assert!(path.exists());
        assert_eq!(config.editor, "code");
        assert_eq!(config.path, path);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn set_keeps_comments_and_handles_tables() {
        let dir = std::env::temp_dir().join(format!("dbx-set-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = fs::remove_dir_all(&dir);
        Config::load_from(&path).unwrap();

        set(&path, "editor", "nvim").unwrap();
        set(&path, "recent_count", 5).unwrap();
        set(&path, "new_project.git_init", false).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# Command used to open a project folder."));
        assert!(text.contains("editor = \"nvim\""));
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.editor, "nvim");
        assert_eq!(config.recent_count, 5);
        assert!(!config.new_project.git_init);
        assert!(config.new_project.create_readme);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tilde_abbreviates_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(tilde(&home.join("Developer")), "~/Developer");
        assert_eq!(tilde(&home), "~");
        assert_eq!(tilde(Path::new("/tmp/x")), "/tmp/x");
    }
}
