//! The Config screen: every setting with its current value. Selecting one
//! edits it in place (toggles flip immediately, others open a prompt or
//! picker), then returns here with the same row highlighted.
//!
//! To expose a new setting: add a variant to `Setting`, list it in `SETTINGS`,
//! and fill in the `match`es.

use std::path::Path;

use anyhow::{Result, bail};

use super::{Action, Ctx, Pick, Prompt, Step, open_in_editor};
use crate::config::{self, Config};
use crate::sys;

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

#[derive(Clone, Copy)]
enum Setting {
    ProjectsDir,
    Editor,
    RecentCount,
    CloseOnOpen,
    GitInit,
    CreateReadme,
}

const SETTINGS: [Setting; 6] = [
    Setting::ProjectsDir,
    Setting::Editor,
    Setting::RecentCount,
    Setting::CloseOnOpen,
    Setting::GitInit,
    Setting::CreateReadme,
];

const LABEL_WIDTH: usize = 23;
const VALUE_WIDTH: usize = 24;

impl Setting {
    fn label(self) -> &'static str {
        match self {
            Setting::ProjectsDir => "Projects folder",
            Setting::Editor => "Editor",
            Setting::RecentCount => "Recent projects shown",
            Setting::CloseOnOpen => "Quit after opening",
            Setting::GitInit => "git init new projects",
            Setting::CreateReadme => "README in new projects",
        }
    }

    /// Key in config.toml (dotted for tables).
    fn key(self) -> &'static str {
        match self {
            Setting::ProjectsDir => "projects_dir",
            Setting::Editor => "editor",
            Setting::RecentCount => "recent_count",
            Setting::CloseOnOpen => "close_on_open",
            Setting::GitInit => "new_project.git_init",
            Setting::CreateReadme => "new_project.create_readme",
        }
    }

    fn display(self, c: &Config) -> String {
        let on_off = |b: bool| if b { "on" } else { "off" }.to_string();
        match self {
            Setting::ProjectsDir => config::tilde(&c.projects_dir),
            Setting::Editor => c.editor.clone(),
            Setting::RecentCount => c.recent_count.to_string(),
            Setting::CloseOnOpen => on_off(c.close_on_open),
            Setting::GitInit => on_off(c.new_project.git_init),
            Setting::CreateReadme => on_off(c.new_project.create_readme),
        }
    }

    /// What happens when the row is selected. `row` is where to return to.
    fn edit(self, ctx: &Ctx, row: usize) -> Result<Step> {
        let c = ctx.config;
        let toggle = |current: bool| -> Result<Step> {
            let updated = save(ctx, self.key(), !current)?;
            Ok(menu(&updated, row))
        };

        match self {
            Setting::CloseOnOpen => toggle(c.close_on_open),
            Setting::GitInit => toggle(c.new_project.git_init),
            Setting::CreateReadme => toggle(c.new_project.create_readme),
            Setting::Editor => Ok(editor_picker(c, row)),

            Setting::ProjectsDir => Ok(Prompt::new("Projects folder", move |text, ctx| {
                let text = text.trim();
                if text.is_empty() {
                    bail!("folder can't be empty");
                }
                let path = config::expand_tilde(Path::new(text))?;
                if !path.is_dir() {
                    bail!("{} is not an existing folder", path.display());
                }
                Ok(menu(&save(ctx, self.key(), text)?, row))
            })
            .initial(config::tilde(&c.projects_dir))
            .back(move |ctx| Ok(menu(ctx.config, row)))
            .into()),

            Setting::RecentCount => Ok(Prompt::new("Recent projects shown", move |text, ctx| {
                let count: i64 = match text.trim().parse() {
                    Ok(n) if n >= 1 => n,
                    _ => bail!("enter a whole number, 1 or more"),
                };
                Ok(menu(&save(ctx, self.key(), count)?, row))
            })
            .initial(c.recent_count.to_string())
            .back(move |ctx| Ok(menu(ctx.config, row)))
            .into()),
        }
    }
}

/// The settings list, with `row` highlighted.
fn menu(config: &Config, row: usize) -> Step {
    let mut items: Vec<String> = SETTINGS
        .iter()
        .map(|s| format!("{:<LABEL_WIDTH$}{}", s.label(), fit(&s.display(config), VALUE_WIDTH)))
        .collect();
    items.push("Open config.toml in editor".into());

    Pick::new("Config", items, |i, ctx| match SETTINGS.get(i) {
        Some(setting) => setting.edit(ctx, i),
        None => {
            let message = format!("opened {}", config::tilde(&ctx.config.path));
            open_in_editor(ctx, &ctx.config.path, Step::Done(message))
        }
    })
    .selected(row)
    .into()
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

fn editor_picker(config: &Config, row: usize) -> Step {
    let current = config.editor.as_str();
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

    Pick::new("Editor", items, move |i, ctx| match EDITOR_PRESETS.get(i) {
        Some((_, cmd)) => save_editor(ctx, cmd, row),
        None => Ok(Prompt::new("Editor command, e.g. code -n", move |cmd, ctx| {
            save_editor(ctx, &cmd, row)
        })
        .initial(ctx.config.editor.clone())
        .back(move |ctx| Ok(editor_picker(ctx.config, row)))
        .into()),
    })
    .selected(selected)
    .back(move |ctx| Ok(menu(ctx.config, row)))
    .into()
}

fn save_editor(ctx: &Ctx, editor: &str, row: usize) -> Result<Step> {
    let editor = editor.trim();
    let Some(program) = editor.split_whitespace().next() else {
        bail!("editor can't be empty");
    };
    // Refuse to save an editor we can't launch; the user stays put and can choose again.
    if sys::find_on_path(program).is_none() {
        bail!("`{program}` not found on PATH");
    }
    Ok(menu(&save(ctx, "editor", editor)?, row))
}

/// Write one key and return the config as it now is on disk, so the menu
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
    use std::fs;

    fn temp_config(name: &str) -> (std::path::PathBuf, Config) {
        let dir = std::env::temp_dir().join(format!("dbx-menu-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = Config::load_from(&dir.join("config.toml")).unwrap();
        (dir, config)
    }

    fn pick_selected(step: Step) -> usize {
        match step {
            Step::Pick(pick) => pick.selected,
            _ => panic!("expected the config menu"),
        }
    }

    #[test]
    fn toggles_flip_each_time_and_return_to_same_row() {
        let (dir, config) = temp_config("toggle");
        assert!(config.close_on_open);

        let step = Setting::CloseOnOpen.edit(&Ctx { config: &config }, 3).unwrap();
        assert_eq!(pick_selected(step), 3);
        let config = config.reload().unwrap();
        assert!(!config.close_on_open);

        Setting::CloseOnOpen.edit(&Ctx { config: &config }, 3).unwrap();
        assert!(config.reload().unwrap().close_on_open);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_invalid_values_without_saving() {
        let (dir, config) = temp_config("invalid");
        let ctx = Ctx { config: &config };

        assert!(save_editor(&ctx, "definitely-not-an-editor", 1).is_err());
        let Ok(Step::Prompt(mut prompt)) = Setting::RecentCount.edit(&ctx, 2) else {
            panic!("expected a prompt");
        };
        assert_eq!(prompt.initial, "10");
        assert!((prompt.on_submit)("0".into(), &ctx).is_err());
        assert!((prompt.on_submit)("abc".into(), &ctx).is_err());

        let reloaded = config.reload().unwrap();
        assert_eq!(reloaded.editor, "code");
        assert_eq!(reloaded.recent_count, 10);

        assert!((prompt.on_submit)(" 25 ".into(), &ctx).is_ok());
        assert_eq!(config.reload().unwrap().recent_count, 25);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fit_keeps_the_end() {
        assert_eq!(fit("short", 10), "short");
        assert_eq!(fit("~/a/very/long/path", 8), "…ng/path");
    }
}
