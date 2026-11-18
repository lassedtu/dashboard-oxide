//! Extensions are bundles of actions with their own settings, switched on
//! and off from Config → Extensions. Each one is stored as a table in
//! config.toml:
//!
//! ```toml
//! [extensions.coding-projects]
//! enabled = true
//! projects_dir = "~/Developer/Projekter"
//! ```
//!
//! Everything optional in dbx is an extension. Config and Quit are core
//! actions and always present.

pub mod coding_projects;
pub mod external;
pub mod folders;
pub mod notes;
pub mod reports;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

use crate::actions::Action;
use crate::config::{Config, Kind, Setting};

pub struct Extension {
    /// Stable id: the table name in config.toml. Never change it once released.
    pub id: String,
    pub name: String,
    /// One short line, shown in the Extensions list.
    pub description: String,
    /// Menu shortcut for the extension's submenu (when it has several actions).
    pub key: char,
    /// Whether it's on when the config doesn't say.
    pub default_enabled: bool,
    /// Shown on its Config page; `load` fills in their defaults.
    pub settings: Vec<Setting>,
    /// One action goes straight in the main menu; several become a submenu.
    pub actions: Vec<Box<dyn Action>>,
    /// Where an external extension is installed; `None` for built-ins.
    pub dir: Option<PathBuf>,
}

impl Extension {
    /// Dotted config key for one of this extension's settings.
    pub fn key_for(&self, setting: &str) -> String {
        format!("extensions.{}.{setting}", self.id)
    }

    /// The `enabled` switch, as a setting so the Config page can show it.
    pub fn enabled_setting(&self) -> Setting {
        Setting::new("enabled", "Enabled", Kind::Toggle, self.default_enabled)
    }

    pub fn is_enabled(&self, config: &Config) -> bool {
        config
            .get(&self.key_for("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(self.default_enabled)
    }

    /// The current value of a setting, or its default.
    pub fn value(&self, config: &Config, setting: &Setting) -> toml::Value {
        config.get(&self.key_for(&setting.key)).cloned().unwrap_or_else(|| setting.default.clone())
    }

    /// Deserialize this extension's settings into its own struct, with defaults
    /// for anything the file leaves out.
    pub fn load<T: DeserializeOwned>(&self, config: &Config) -> Result<T> {
        let table: toml::Table = self
            .settings
            .iter()
            .map(|s| (s.key.clone(), self.value(config, s)))
            .collect();
        toml::Value::Table(table)
            .try_into()
            .with_context(|| format!("reading [extensions.{}]", self.id))
    }

    /// Check the types of everything in this extension's table.
    pub fn check(&self, config: &Config) -> Result<()> {
        for setting in std::iter::once(self.enabled_setting()).chain(self.settings.iter().cloned()) {
            if let Some(value) = config.get(&self.key_for(&setting.key)) {
                setting.check(value).with_context(|| format!("[extensions.{}]", self.id))?;
            }
        }
        Ok(())
    }
}

/// Every known extension, enabled or not, in menu order.
pub fn all(config: &Config) -> Vec<Extension> {
    all_at(&config.path)
}

/// Like `all`, for the config file at `config_path` (which may not be loaded yet).
/// Built-ins first, then installed extensions by id.
pub fn all_at(config_path: &Path) -> Vec<Extension> {
    let mut all = builtin();
    let ids: Vec<String> = all.iter().map(|e| e.id.clone()).collect();
    all.extend(external::discover(config_path, &ids).0);
    all
}

/// Installed extensions that couldn't be loaded, with the reason.
pub fn broken(config: &Config) -> Vec<external::Broken> {
    let ids: Vec<String> = builtin().iter().map(|e| e.id.clone()).collect();
    external::discover(&config.path, &ids).1
}

fn builtin() -> Vec<Extension> {
    vec![coding_projects::extension(), reports::extension(), notes::extension()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_and_keys_are_unique_and_settings_load() {
        let all = all(&Config::default());
        let ids: HashSet<_> = all.iter().map(|e| &e.id).collect();
        assert_eq!(ids.len(), all.len());
        for ext in &all {
            assert!(!ext.id.contains('.'), "ids are config keys: {}", ext.id);
            assert!(!matches!(ext.key, 'c' | 'q' | 'j' | 'k'), "{} uses a reserved key", ext.id);
            for setting in &ext.settings {
                setting.check(&setting.default).unwrap();
            }
        }
    }

    #[test]
    fn enabled_follows_config_then_default() {
        let ext = coding_projects::extension();
        let mut config = Config::default();
        assert!(ext.is_enabled(&config));
        config.raw = toml::from_str("[extensions.coding-projects]\nenabled = false").unwrap();
        assert!(!ext.is_enabled(&config));
    }
}
