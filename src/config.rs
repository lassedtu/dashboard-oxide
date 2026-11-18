use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::{extensions, sys};

/// Written to ~/.dbx/config.toml on first run. Must stay in sync with
/// `core_settings()` and the extensions' defaults (enforced by tests).
const DEFAULT_CONFIG: &str = r#"# dbx configuration

# Command used to open projects and files. May include arguments, e.g. "code -n".
# Terminal editors (nvim, vim, hx, ...) take over the terminal while open.
editor = "code"

# Quit dbx after opening something in the editor.
close_on_open = true

# Each extension has its own table. Leave a key out to use its default.
# Everything here can also be changed from Config -> Extensions inside dbx.

[extensions.coding-projects]
enabled = true
# Folder where new projects are created and recent projects are listed from.
projects_dir = "~/Developer/Projekter"
# How many projects to show under "Recent projects".
recent_count = 10
# What to set up in new projects.
git_init = true
create_readme = true

[extensions.reports]
enabled = false
# Author on new reports.
name = ""
# One folder per course, named like "02323 Introduction to Statistics".
root = "~/Documents/DTU"
# Folder inside each course where reports are created.
assignments_dir = "Assignments"
# How many reports to show under "Recent reports".
recent_count = 10
"#;

/// The core settings, i.e. everything not owned by an extension.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(default)] // missing keys fall back to Default, so old configs keep working
pub struct Config {
    /// Where this config was loaded from (not a setting).
    #[serde(skip)]
    pub path: PathBuf,
    /// The whole file, for extension settings (see `Extension::load`).
    #[serde(skip)]
    pub raw: toml::Table,
    pub editor: String,
    pub close_on_open: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            raw: toml::Table::new(),
            editor: "code".into(),
            close_on_open: true,
        }
    }
}

/// Core settings as shown on the Config screen. Defaults must match `Config::default()`.
pub fn core_settings() -> Vec<Setting> {
    vec![
        Setting::new("editor", "Editor", Kind::Editor, "code"),
        Setting::new("close_on_open", "Quit after opening", Kind::Toggle, true),
    ]
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
        migrate(path)?;

        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut config: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        config.raw = toml::from_str(&text)?;
        config.path = path.to_path_buf();

        // Catch wrong types now rather than when an extension runs.
        for ext in extensions::all() {
            ext.check(&config).with_context(|| format!("in {}", path.display()))?;
        }
        Ok(config)
    }

    /// Look up a dotted key in the file, e.g. `"extensions.reports.name"`.
    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        let (tables, name) = split_key(key);
        let mut table = &self.raw;
        for part in tables {
            table = table.get(part)?.as_table()?;
        }
        table.get(name)
    }
}

/// How a setting is shown, validated and edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Any non-empty text.
    Text,
    /// An existing folder; `~` is allowed and kept as typed.
    Folder,
    /// A whole number, 1 or more.
    Count,
    Toggle,
    /// An editor command; its program must be on PATH.
    Editor,
}

/// One user-facing setting: its key in its table, label, kind and default.
#[derive(Clone, Debug)]
pub struct Setting {
    pub key: String,
    pub label: String,
    pub kind: Kind,
    pub default: toml::Value,
}

impl Setting {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        kind: Kind,
        default: impl Into<toml::Value>,
    ) -> Self {
        Self { key: key.into(), label: label.into(), kind, default: default.into() }
    }

    /// Check a value from the file has the right type.
    pub fn check(&self, value: &toml::Value) -> Result<()> {
        let ok = match self.kind {
            Kind::Text | Kind::Folder | Kind::Editor => value.is_str(),
            Kind::Count => value.as_integer().is_some_and(|n| n >= 1),
            Kind::Toggle => value.is_bool(),
        };
        if !ok {
            let expected = match self.kind {
                Kind::Text | Kind::Folder | Kind::Editor => "text in quotes",
                Kind::Count => "a whole number, 1 or more",
                Kind::Toggle => "true or false",
            };
            bail!("`{}` should be {expected}, found {value}", self.key);
        }
        Ok(())
    }

    /// Validate what the user typed on the Config screen and turn it into a value to save.
    pub fn parse(&self, input: &str) -> Result<toml_edit::Value> {
        let input = input.trim();
        Ok(match self.kind {
            Kind::Text => {
                if input.is_empty() {
                    bail!("{} can't be empty", self.label);
                }
                input.into()
            }
            Kind::Folder => {
                if input.is_empty() {
                    bail!("folder can't be empty");
                }
                let path = expand_tilde(Path::new(input))?;
                if !path.is_dir() {
                    bail!("{} is not an existing folder", path.display());
                }
                input.into()
            }
            Kind::Count => match input.parse::<i64>() {
                Ok(n) if n >= 1 => n.into(),
                _ => bail!("enter a whole number, 1 or more"),
            },
            Kind::Toggle => match input {
                "on" | "true" => true.into(),
                "off" | "false" => false.into(),
                _ => bail!("enter on or off"),
            },
            Kind::Editor => {
                let Some(program) = input.split_whitespace().next() else {
                    bail!("editor can't be empty");
                };
                // Refuse to save an editor we can't launch.
                if sys::find_on_path(program).is_none() {
                    bail!("`{program}` not found on PATH");
                }
                input.into()
            }
        })
    }

    /// The value as shown on the Config screen and pre-filled when editing.
    pub fn display(&self, value: &toml::Value) -> String {
        match (self.kind, value) {
            (Kind::Toggle, toml::Value::Boolean(b)) => if *b { "on" } else { "off" }.into(),
            (Kind::Folder, toml::Value::String(s)) => match expand_tilde(Path::new(s)) {
                Ok(path) => tilde(&path),
                Err(_) => s.clone(),
            },
            (_, toml::Value::String(s)) => s.clone(),
            (_, other) => other.to_string(),
        }
    }
}

fn split_key(key: &str) -> (Vec<&str>, &str) {
    match key.rsplit_once('.') {
        Some((tables, name)) => (tables.split('.').collect(), name),
        None => (Vec::new(), key),
    }
}

/// Set one key in the config file at `path`, keeping comments and formatting.
/// `key` may be dotted for tables, e.g. `"extensions.reports.name"`; missing
/// tables are created.
pub fn set(path: &Path, key: &str, value: impl Into<toml_edit::Value>) -> Result<()> {
    let mut doc = read_doc(path)?;
    let (tables, name) = split_key(key);
    let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
    for part in tables {
        table = table
            .entry(part)
            .or_insert_with(implicit_table)
            .as_table_like_mut()
            .with_context(|| format!("`{part}` in {} is not a table", path.display()))?;
    }
    match table.get_mut(name) {
        // Replace in place, so the comment above the key is kept.
        Some(item) => *item = toml_edit::value(value),
        None => {
            table.insert(name, toml_edit::value(value));
        }
    }
    write_doc(path, &doc)
}

/// A table that only gets a `[header]` once it has keys of its own, so
/// `[extensions.foo]` doesn't come with an empty `[extensions]` above it.
fn implicit_table() -> toml_edit::Item {
    let mut table = toml_edit::Table::new();
    table.set_implicit(true);
    toml_edit::Item::Table(table)
}

fn read_doc(path: &Path) -> Result<toml_edit::DocumentMut> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    text.parse().with_context(|| format!("parsing {}", path.display()))
}

fn write_doc(path: &Path, doc: &toml_edit::DocumentMut) -> Result<()> {
    fs::write(path, doc.to_string()).with_context(|| format!("writing {}", path.display()))
}

/// Move settings from older layouts to where they live now. Comments move
/// with their keys. Runs on every load and does nothing once migrated.
fn migrate(path: &Path) -> Result<()> {
    let mut doc = read_doc(path)?;
    let root = doc.as_table_mut();

    // Before extensions, the coding-project settings were top-level.
    let mut moved = Vec::new();
    for key in ["projects_dir", "recent_count"] {
        if let Some(entry) = root.remove_entry(key) {
            moved.push(entry);
        }
    }
    if let Some(toml_edit::Item::Table(mut old)) = root.remove("new_project") {
        for key in ["git_init", "create_readme"] {
            if let Some(entry) = old.remove_entry(key) {
                moved.push(entry);
            }
        }
    }
    if moved.is_empty() {
        return Ok(());
    }

    let extensions = root
        .entry("extensions")
        .or_insert_with(implicit_table)
        .as_table_mut()
        .context("`extensions` is not a table")?;
    let target = extensions
        .entry("coding-projects")
        .or_insert(toml_edit::table())
        .as_table_mut()
        .context("`extensions.coding-projects` is not a table")?;
    for (key, item) in moved {
        if !target.contains_key(key.get()) {
            target.insert_formatted(&key, item);
        }
    }
    write_doc(path, &doc)
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
pub mod tests {
    use super::*;

    /// A fresh config file in a temp dir, unique per test name.
    pub fn temp_config(name: &str) -> (PathBuf, Config) {
        let dir = std::env::temp_dir().join(format!("dbx-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = Config::load_from(&dir.join("config.toml")).unwrap();
        (dir, config)
    }

    #[test]
    fn template_matches_defaults() {
        let template: toml::Table = toml::from_str(DEFAULT_CONFIG).unwrap();
        let parsed: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(parsed, Config::default());
        for setting in core_settings() {
            assert_eq!(template.get(&setting.key), Some(&setting.default), "{}", setting.key);
        }

        // Every extension table in the template spells out exactly the defaults.
        let tables = template["extensions"].as_table().unwrap();
        for ext in extensions::all() {
            let Some(table) = tables.get(&ext.id).and_then(|t| t.as_table()) else { continue };
            assert_eq!(table.get("enabled").and_then(|v| v.as_bool()), Some(ext.default_enabled));
            for setting in &ext.settings {
                assert_eq!(table.get(&setting.key), Some(&setting.default), "{}", setting.key);
            }
        }
    }

    #[test]
    fn missing_keys_use_defaults() {
        let parsed: Config = toml::from_str("close_on_open = false").unwrap();
        assert!(!parsed.close_on_open);
        assert_eq!(parsed.editor, "code");
    }

    #[test]
    fn expands_tilde() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde(Path::new("~/foo")).unwrap(), home.join("foo"));
        assert_eq!(expand_tilde(Path::new("/abs")).unwrap(), PathBuf::from("/abs"));
    }

    #[test]
    fn creates_file_on_first_load() {
        let (dir, config) = temp_config("first-load");
        assert!(config.path.exists());
        assert_eq!(config.editor, "code");
        assert_eq!(config.get("extensions.coding-projects.recent_count"), Some(&10.into()));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn set_keeps_comments_and_creates_tables() {
        let (dir, config) = temp_config("set");
        let path = &config.path;

        set(path, "editor", "nvim").unwrap();
        set(path, "extensions.coding-projects.recent_count", 5).unwrap();
        set(path, "extensions.other.enabled", false).unwrap();

        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("# Command used to open projects and files."));
        assert!(text.contains("# How many projects to show"));
        assert!(text.contains("[extensions.other]"));
        assert!(!text.contains("[extensions]\n"), "no empty parent header:\n{text}");

        let config = config.reload().unwrap();
        assert_eq!(config.editor, "nvim");
        assert_eq!(config.get("extensions.coding-projects.recent_count"), Some(&5.into()));
        assert_eq!(config.get("extensions.other.enabled"), Some(&false.into()));

        fs::write(path, "extensions = 3\n").unwrap();
        assert!(set(path, "extensions.other.enabled", false).is_err());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn migrates_top_level_project_settings() {
        let dir = std::env::temp_dir().join(format!("dbx-migrate-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &path,
            "editor = \"vim\"\n\n# Where projects live.\nprojects_dir = \"/tmp\"\nrecent_count = 3\n\n[new_project]\ngit_init = false\n",
        )
        .unwrap();

        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.editor, "vim");
        assert_eq!(config.get("extensions.coding-projects.projects_dir"), Some(&"/tmp".into()));
        assert_eq!(config.get("extensions.coding-projects.recent_count"), Some(&3.into()));
        assert_eq!(config.get("extensions.coding-projects.git_init"), Some(&false.into()));
        assert_eq!(config.get("projects_dir"), None);

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# Where projects live.\nprojects_dir"), "{text}");
        assert!(!text.contains("[new_project]"));

        // Running again changes nothing.
        Config::load_from(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), text);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_wrong_types_on_load() {
        let dir = std::env::temp_dir().join(format!("dbx-types-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "[extensions.coding-projects]\nrecent_count = \"ten\"\n").unwrap();

        let err = format!("{:#}", Config::load_from(&path).unwrap_err());
        assert!(err.contains("recent_count"), "{err}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parses_and_displays_by_kind() {
        let count = Setting::new("n", "N", Kind::Count, 1);
        assert_eq!(count.parse(" 7 ").unwrap().as_integer(), Some(7));
        assert!(count.parse("0").is_err());
        assert!(Setting::new("t", "Name", Kind::Text, "").parse("  ").is_err());
        assert!(Setting::new("f", "F", Kind::Folder, "").parse("/definitely/not/here").is_err());
        assert!(Setting::new("e", "E", Kind::Editor, "").parse("not-an-editor-xyz").is_err());

        let home = dirs::home_dir().unwrap();
        let folder = Setting::new("f", "F", Kind::Folder, "~");
        let shown = folder.display(&home.join("x").display().to_string().into());
        assert_eq!(shown, "~/x");
        let toggle = Setting::new("t", "T", Kind::Toggle, true);
        assert_eq!(toggle.display(&false.into()), "off");
    }

    #[test]
    fn tilde_abbreviates_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(tilde(&home.join("Developer")), "~/Developer");
        assert_eq!(tilde(&home), "~");
        assert_eq!(tilde(Path::new("/tmp/x")), "/tmp/x");
    }
}
