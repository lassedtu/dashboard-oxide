//! The Config screen:
//!
//! ```text
//! Config                     core settings, Extensions, open config.toml
//!   └ Extensions             every extension, on or off
//!       └ <extension>        Enabled + that extension's settings
//! ```
//!
//! Every page is a list of `Setting`s (see config.rs), so there is no
//! per-setting code here: the setting's `Kind` decides how it's shown and
//! edited. After an edit, the page reopens with the same row highlighted.

use std::rc::Rc;

use anyhow::{Result, bail};

use super::{Action, Ctx, Pick, Prompt, Step, open_in_editor};
use crate::config::{self, Config, Kind, Setting};
use crate::extensions;

pub struct ConfigMenu;

impl Action for ConfigMenu {
    fn key(&self) -> char {
        'c'
    }

    fn title(&self) -> &str {
        "Config"
    }

    fn run(&self, ctx: &Ctx) -> Result<Step> {
        Ok(menu(ctx.config, 0))
    }
}

const LABEL_WIDTH: usize = 23;
const VALUE_WIDTH: usize = 24;

/// Rebuilds the page an edit started from, with the given config.
type Reopen = Rc<dyn Fn(&Config) -> Step>;

/// A setting together with the full dotted key it's stored under.
#[derive(Clone)]
struct Entry {
    key: String,
    setting: Setting,
}

impl Entry {
    fn value(&self, config: &Config) -> toml::Value {
        config.get(&self.key).cloned().unwrap_or_else(|| self.setting.default.clone())
    }

    fn row(&self, config: &Config) -> String {
        let value = self.setting.display(&self.value(config));
        let value = if value.is_empty() { "not set".into() } else { value };
        row(&self.setting.label, &value)
    }
}

fn row(label: &str, value: &str) -> String {
    format!("{label:<LABEL_WIDTH$}{}", fit(value, VALUE_WIDTH))
}

/// The top-level Config page.
fn menu(config: &Config, selected: usize) -> Step {
    let entries: Vec<Entry> = config::core_settings()
        .into_iter()
        .map(|setting| Entry { key: setting.key.clone(), setting })
        .collect();

    let all = extensions::all();
    let on = all.iter().filter(|e| e.is_enabled(config)).count();
    let mut items: Vec<String> = entries.iter().map(|e| e.row(config)).collect();
    items.push(row("Extensions", &format!("{on} of {} on", all.len())));
    items.push("Open config.toml in editor".into());

    Pick::new("Config", items, move |i, ctx| {
        if let Some(entry) = entries.get(i) {
            return edit(entry.clone(), ctx, Rc::new(move |c| menu(c, i)));
        }
        if i == entries.len() {
            return Ok(extensions_list(ctx.config, i, 0));
        }
        let message = format!("opened {}", config::tilde(&ctx.config.path));
        open_in_editor(ctx, &ctx.config.path, Step::Done(message))
    })
    .selected(selected)
    .into()
}

/// Every extension and whether it's on. `menu_row` is where Esc returns to.
fn extensions_list(config: &Config, menu_row: usize, selected: usize) -> Step {
    let all = extensions::all();
    let items = all
        .iter()
        .map(|e| {
            let state = if e.is_enabled(config) { "on" } else { "off" };
            format!("{state:<5}{:<18}{}", e.name, fit(&e.description, VALUE_WIDTH))
        })
        .collect();
    let ids: Vec<String> = all.into_iter().map(|e| e.id).collect();

    Pick::new("Extensions", items, move |i, ctx| {
        Ok(extension_page(ctx.config, &ids[i], menu_row, i, 0))
    })
    .selected(selected)
    .back(move |ctx| Ok(menu(ctx.config, menu_row)))
    .into()
}

/// One extension: its on/off switch followed by its settings.
fn extension_page(config: &Config, id: &str, menu_row: usize, list_row: usize, selected: usize) -> Step {
    let Some(ext) = extensions::all().into_iter().find(|e| e.id == id) else {
        return extensions_list(config, menu_row, list_row);
    };
    let entries: Vec<Entry> = std::iter::once(ext.enabled_setting())
        .chain(ext.settings.iter().cloned())
        .map(|setting| Entry { key: ext.key_for(&setting.key), setting })
        .collect();
    let items = entries.iter().map(|e| e.row(config)).collect();

    let id = id.to_string();
    Pick::new(ext.name, items, move |i, ctx| {
        let id = id.clone();
        let reopen = Rc::new(move |c: &Config| extension_page(c, &id, menu_row, list_row, i));
        edit(entries[i].clone(), ctx, reopen)
    })
    .selected(selected)
    .back(move |ctx| Ok(extensions_list(ctx.config, menu_row, list_row)))
    .into()
}

/// What selecting a setting does: toggles flip in place, editors get a
/// picker, everything else a pre-filled prompt. Ends by reopening the page.
fn edit(entry: Entry, ctx: &Ctx, reopen: Reopen) -> Result<Step> {
    let current = entry.value(ctx.config);
    match entry.setting.kind {
        Kind::Toggle => {
            let updated = save(ctx, &entry.key, !current.as_bool().unwrap_or(false))?;
            Ok(reopen(&updated))
        }
        Kind::Editor => Ok(editor_picker(ctx.config, entry, reopen)),
        Kind::Text | Kind::Folder | Kind::Count => {
            let initial = entry.setting.display(&current);
            let back = reopen.clone();
            Ok(Prompt::new(entry.setting.label.clone(), move |text, ctx| {
                let value = entry.setting.parse(&text)?;
                Ok(reopen(&save(ctx, &entry.key, value)?))
            })
            .initial(initial)
            .back(move |ctx| Ok(back(ctx.config)))
            .into())
        }
    }
}

/// Shown in the picker as (label, command). Anything else goes through "Custom…".
const EDITOR_PRESETS: &[(&str, &str)] = &[
    ("VS Code", "code"),
    ("Cursor", "cursor"),
    ("Zed", "zed"),
    ("Sublime Text", "subl"),
    ("Neovim", "nvim"),
    ("Vim", "vim"),
    ("Helix", "hx"),
];

fn editor_picker(config: &Config, entry: Entry, reopen: Reopen) -> Step {
    let current = entry.value(config);
    let current = current.as_str().unwrap_or_default();
    let mut items: Vec<String> = EDITOR_PRESETS
        .iter()
        .map(|(label, cmd)| {
            let mark = if *cmd == current { "  ✓ current" } else { "" };
            format!("{label:<14}{cmd}{mark}")
        })
        .collect();
    items.push("Custom…".into());
    let selected = EDITOR_PRESETS
        .iter()
        .position(|(_, cmd)| *cmd == current)
        .unwrap_or(EDITOR_PRESETS.len());

    let back = reopen.clone();
    Pick::new(entry.setting.label.clone(), items, move |i, ctx| {
        let (entry, reopen) = (entry.clone(), reopen.clone());
        match EDITOR_PRESETS.get(i) {
            Some((_, cmd)) => save_editor(ctx, &entry, cmd, &reopen),
            None => {
                let again = (entry.clone(), reopen.clone());
                let initial = entry.setting.display(&entry.value(ctx.config));
                Ok(Prompt::new("Editor command, e.g. code -n", move |cmd, ctx| {
                    save_editor(ctx, &entry, &cmd, &reopen)
                })
                .initial(initial)
                .back(move |ctx| Ok(editor_picker(ctx.config, again.0.clone(), again.1.clone())))
                .into())
            }
        }
    })
    .selected(selected)
    .back(move |ctx| Ok(back(ctx.config)))
    .into()
}

fn save_editor(ctx: &Ctx, entry: &Entry, editor: &str, reopen: &Reopen) -> Result<Step> {
    if entry.setting.kind != Kind::Editor {
        bail!("{} is not an editor setting", entry.setting.label);
    }
    let value = entry.setting.parse(editor)?;
    Ok(reopen(&save(ctx, &entry.key, value)?))
}

/// Write one key and return the config as it now is on disk, so the page
/// we return to shows the new value.
fn save(ctx: &Ctx, key: &str, value: impl Into<toml_edit::Value>) -> Result<Config> {
    config::set(&ctx.config.path, key, value)?;
    ctx.config.reload()
}

/// Shorten from the left so the meaningful end of a path stays visible.
fn fit(value: &str, width: usize) -> String {
    let len = value.chars().count();
    if len <= width {
        return value.to_string();
    }
    let tail: String = value.chars().skip(len - (width - 1)).collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::tests::temp_config;
    use crate::extensions::coding_projects;
    use std::fs;

    fn pick(step: Step) -> crate::actions::Pick {
        match step {
            Step::Pick(pick) => pick,
            _ => panic!("expected a list"),
        }
    }

    fn select(step: Step, row: usize, config: &Config) -> Result<Step> {
        (pick(step).on_select)(row, &Ctx { config })
    }

    #[test]
    fn core_toggle_flips_each_time_and_returns_to_same_row() {
        let (dir, config) = temp_config("menu-toggle");
        assert!(config.close_on_open);

        let step = select(menu(&config, 0), 1, &config).unwrap();
        assert_eq!(pick(step).selected, 1);
        let config = config.reload().unwrap();
        assert!(!config.close_on_open);

        select(menu(&config, 0), 1, &config).unwrap();
        assert!(config.reload().unwrap().close_on_open);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn extension_page_edits_its_own_table() {
        let (dir, config) = temp_config("menu-ext");
        let page = |c: &Config, row| extension_page(c, coding_projects::ID, 2, 0, row);
        assert!(pick(page(&config, 0)).items[1].contains("~/Developer/Projekter"));

        // Row 0 is Enabled.
        let step = select(page(&config, 0), 0, &config).unwrap();
        assert!(pick(step).items[0].ends_with("off"));
        let config = config.reload().unwrap();
        assert_eq!(config.get("extensions.coding-projects.enabled"), Some(&false.into()));

        // Row 2 is the recent count: invalid input is refused, valid input saved.
        let Step::Prompt(mut prompt) = select(page(&config, 0), 2, &config).unwrap() else {
            panic!("expected a prompt");
        };
        assert_eq!(prompt.initial, "10");
        let ctx = Ctx { config: &config };
        assert!((prompt.on_submit)("0".into(), &ctx).is_err());
        let count = |c: &Config| c.reload().unwrap().get("extensions.coding-projects.recent_count").cloned();
        assert_eq!(count(&config), Some(10.into()));
        let step = (prompt.on_submit)(" 25 ".into(), &ctx).unwrap();
        assert_eq!(pick(step).selected, 2);
        assert_eq!(count(&config), Some(25.into()));

        // Esc goes back to the Extensions list.
        let back = (pick(page(&config, 0)).on_back.unwrap())(&ctx).unwrap();
        assert_eq!(pick(back).title, "Extensions");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn editor_must_exist() {
        let (dir, config) = temp_config("menu-editor");
        let entry = Entry { key: "editor".into(), setting: config::core_settings().remove(0) };
        let reopen: Reopen = Rc::new(|c| menu(c, 0));
        let ctx = Ctx { config: &config };
        assert!(save_editor(&ctx, &entry, "definitely-not-an-editor", &reopen).is_err());
        assert_eq!(config.reload().unwrap().editor, "code");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fit_keeps_the_end() {
        assert_eq!(fit("short", 10), "short");
        assert_eq!(fit("~/a/very/long/path", 8), "…ng/path");
    }
}
