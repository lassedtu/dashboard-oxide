//! Extensions installed as folders next to the config file:
//!
//! ```text
//! ~/.dbx/extensions/<id>/extension.toml   what it is, its settings and actions
//! ~/.dbx/extensions/<id>/...              the scripts it runs
//! ```
//!
//! Each action runs a shell command in the extension's folder. dbx passes the
//! user's input and the extension's settings as environment variables, and
//! shows the last line the command prints (or its error) as the status.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::Extension;
use crate::actions::{Action, Ctx, Step};
use crate::config::{self, Config, Kind, Setting};
use crate::sys;

pub const MANIFEST: &str = "extension.toml";

/// An extension folder that couldn't be loaded, shown in Config → Extensions.
pub struct Broken {
    pub id: String,
    pub error: String,
}

/// The extensions folder for the config file at `config_path`.
pub fn dir_for(config_path: &Path) -> Option<PathBuf> {
    let parent = config_path.parent()?;
    (!parent.as_os_str().is_empty()).then(|| parent.join("extensions"))
}

/// Every extension folder, loaded or with the reason it couldn't be. By id.
pub fn discover(config_path: &Path, taken: &[String]) -> (Vec<Extension>, Vec<Broken>) {
    let (mut found, mut broken) = (Vec::new(), Vec::new());
    let Some(dir) = dir_for(config_path) else { return (found, broken) };
    let Ok(entries) = fs::read_dir(&dir) else { return (found, broken) };

    let mut folders: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| Some((e.file_name().into_string().ok()?, e.path())))
        .filter(|(id, _)| !id.starts_with('.'))
        .collect();
    folders.sort();

    for (id, path) in folders {
        let result = if taken.contains(&id) {
            Err(anyhow::anyhow!("the id `{id}` is already used by a built-in extension"))
        } else {
            load(&id, &path)
        };
        match result {
            Ok(ext) => found.push(ext),
            Err(err) => broken.push(Broken { id, error: format!("{err:#}") }),
        }
    }
    (found, broken)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
    #[serde(default)]
    description: String,
    /// Menu key; needed when there are several actions (they get a submenu).
    key: Option<String>,
    #[serde(default = "yes")]
    default_enabled: bool,
    #[serde(default)]
    settings: Vec<ManifestSetting>,
    actions: Vec<ManifestAction>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestSetting {
    key: String,
    label: String,
    kind: Kind,
    default: toml::Value,
    #[serde(default)]
    help: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestAction {
    title: String,
    key: String,
    /// Shell command, run with `sh -c` in the extension's folder.
    run: String,
    /// Ask for a line of text first; passed as $DBX_INPUT.
    prompt: Option<String>,
    /// Give the command the terminal (for interactive programs).
    #[serde(default)]
    terminal: bool,
}

fn load(id: &str, dir: &Path) -> Result<Extension> {
    if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        bail!("folder name `{id}` should only use a-z, 0-9, - and _ (it's the extension's id)");
    }
    let path = dir.join(MANIFEST);
    let text = fs::read_to_string(&path).with_context(|| format!("no {MANIFEST} in {}", dir.display()))?;
    let manifest: Manifest = toml::from_str(&text).with_context(|| format!("in {}", path.display()))?;

    if manifest.actions.is_empty() {
        bail!("{MANIFEST} has no [[actions]]");
    }
    let settings = manifest
        .settings
        .into_iter()
        .map(|s| {
            if s.key == "enabled" || !s.key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                bail!("setting key `{}` should only use letters, digits and _, and not be `enabled`", s.key);
            }
            let setting = Setting::new(s.key, s.label, s.kind, s.default).help(s.help);
            setting.check(&setting.default).context("its default")?;
            Ok(setting)
        })
        .collect::<Result<Vec<_>>>()?;

    let in_main_menu = manifest.actions.len() == 1;
    let key = match manifest.key.as_deref() {
        Some(key) => menu_key(key, true)?,
        None if in_main_menu => menu_key(&manifest.actions[0].key, true)?,
        None => bail!("an extension with several actions needs a `key` for its submenu"),
    };

    let actions = manifest
        .actions
        .into_iter()
        .map(|a| -> Result<Box<dyn Action>> {
            Ok(Box::new(ExternalAction(Rc::new(Spec {
                id: id.to_string(),
                dir: dir.to_path_buf(),
                key: menu_key(&a.key, in_main_menu)?,
                title: a.title,
                run: a.run,
                prompt: a.prompt,
                terminal: a.terminal,
                settings: settings.clone(),
            }))))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Extension {
        id: id.to_string(),
        name: manifest.name,
        description: manifest.description,
        key,
        default_enabled: manifest.default_enabled,
        settings,
        actions,
        dir: Some(dir.to_path_buf()),
    })
}

/// One character, not a movement key, and in the main menu not Config's or Quit's.
fn menu_key(key: &str, main_menu: bool) -> Result<char> {
    let mut chars = key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        bail!("key `{key}` should be a single character");
    };
    if matches!(c, 'j' | 'k') || (main_menu && matches!(c, 'c' | 'q')) {
        bail!("key `{c}` is reserved (j/k move, c is Config, q is Quit)");
    }
    Ok(c)
}

struct Spec {
    id: String,
    dir: PathBuf,
    key: char,
    title: String,
    run: String,
    prompt: Option<String>,
    terminal: bool,
    settings: Vec<Setting>,
}

struct ExternalAction(Rc<Spec>);

impl Action for ExternalAction {
    fn key(&self) -> char {
        self.0.key
    }

    fn title(&self) -> &str {
        &self.0.title
    }

    fn run(&self, ctx: &Ctx) -> Result<Step> {
        match &self.0.prompt {
            Some(label) => {
                let spec = self.0.clone();
                Ok(Step::prompt(label.clone(), move |input, ctx| execute(&spec, &input, ctx)))
            }
            None => execute(&self.0, "", ctx),
        }
    }
}

fn execute(spec: &Spec, input: &str, ctx: &Ctx) -> Result<Step> {
    let command = command(spec, input, ctx.config)?;
    if spec.terminal {
        let then = Step::Done(format!("{} finished", spec.title));
        return Ok(Step::RunInTerminal { command, then: Box::new(then) });
    }
    let output = sys::run_captured(command)?;
    let message = output
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map_or_else(|| format!("{} done", spec.title), str::to_string);
    Ok(Step::Done(message))
}

/// `sh -c <run>` in the extension's folder, with:
///
/// - `DBX_INPUT`: what the user typed (if the action has a `prompt`)
/// - `DBX_SETTING_<KEY>`: each setting, e.g. `DBX_SETTING_TODO_FILE` (folders with `~` expanded)
/// - `DBX_EDITOR`, `DBX_CONFIG`, `DBX_EXTENSION_DIR`
fn command(spec: &Spec, input: &str, config: &Config) -> Result<Command> {
    let mut command = Command::new("sh");
    command.arg("-c").arg(&spec.run).current_dir(&spec.dir);
    command
        .env("DBX_INPUT", input)
        .env("DBX_EDITOR", &config.editor)
        .env("DBX_CONFIG", &config.path)
        .env("DBX_EXTENSION_DIR", &spec.dir);

    for setting in &spec.settings {
        let key = format!("extensions.{}.{}", spec.id, setting.key);
        let value = config.get(&key).cloned().unwrap_or_else(|| setting.default.clone());
        let text = match (&value, setting.kind) {
            (toml::Value::String(s), Kind::Folder) => {
                config::expand_tilde(Path::new(s))?.display().to_string()
            }
            (toml::Value::String(s), _) => s.clone(),
            (other, _) => other.to_string(),
        };
        command.env(format!("DBX_SETTING_{}", setting.key.to_uppercase()), text);
    }
    Ok(command)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::config::tests::temp_config;

    /// Write `~/.dbx/extensions/<id>/extension.toml` (and a script) next to `config`.
    pub fn install(config: &Config, id: &str, manifest: &str, script: &str) -> PathBuf {
        let dir = dir_for(&config.path).unwrap().join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(MANIFEST), manifest).unwrap();
        fs::write(dir.join("run.sh"), script).unwrap();
        dir
    }

    const TODO: &str = r#"
name = "Todo"
description = "Add a todo"

[[settings]]
key = "greeting"
label = "Greeting"
kind = "text"
default = "added"

[[actions]]
title = "Add todo"
key = "t"
run = "sh run.sh"
prompt = "Todo"
"#;

    #[test]
    fn runs_command_with_input_and_settings() {
        let (dir, config) = temp_config("ext-run");
        install(&config, "todo", TODO, "test -n \"$DBX_INPUT\" || { echo 'empty todo' >&2; exit 1; }\necho \"$DBX_INPUT\" >> todo.txt\necho \"$DBX_SETTING_GREETING: $DBX_INPUT\"\n");
        let config = config.reload().unwrap();

        let (found, broken) = discover(&config.path, &[]);
        assert!(broken.is_empty(), "{}", broken[0].error);
        let ext = &found[0];
        assert_eq!((ext.key, ext.name.as_str()), ('t', "Todo"));
        // The config file got a table for it.
        assert_eq!(config.get("extensions.todo.greeting"), Some(&"added".into()));

        let ctx = Ctx { config: &config };
        let Step::Prompt(mut prompt) = ext.actions[0].run(&ctx).unwrap() else { panic!() };
        let Step::Done(message) = (prompt.on_submit)("buy milk".into(), &ctx).unwrap() else {
            panic!()
        };
        assert_eq!(message, "added: buy milk");
        let todo = dir_for(&config.path).unwrap().join("todo/todo.txt");
        assert_eq!(fs::read_to_string(todo).unwrap(), "buy milk\n");

        // A failing command shows its stderr and leaves you on the prompt.
        let err = (prompt.on_submit)("".into(), &ctx).err().expect("an error");
        assert_eq!(format!("{err:#}"), "empty todo");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bad_extensions_are_reported_not_fatal() {
        let (dir, config) = temp_config("ext-broken");
        install(&config, "nokey", "name = \"X\"\n[[actions]]\ntitle = \"A\"\nkey = \"a\"\nrun = \"true\"\n[[actions]]\ntitle = \"B\"\nkey = \"b\"\nrun = \"true\"\n", "");
        install(&config, "quitter", "name = \"X\"\n[[actions]]\ntitle = \"A\"\nkey = \"q\"\nrun = \"true\"\n", "");
        install(&config, "typo", "name = \"X\"\nacshuns = []\n", "");
        install(&config, "Bad.Id", TODO, "");
        fs::create_dir_all(dir_for(&config.path).unwrap().join("empty")).unwrap();
        install(&config, "coding-projects", TODO, "");

        let (found, broken) = discover(&config.path, &["coding-projects".into()]);
        assert!(found.is_empty());
        let errors: Vec<_> = broken.iter().map(|b| (b.id.as_str(), b.error.as_str())).collect();
        let error = |id| errors.iter().find(|(i, _)| *i == id).unwrap().1;
        assert!(error("nokey").contains("needs a `key`"));
        assert!(error("quitter").contains("reserved"));
        assert!(error("typo").contains("acshuns"));
        assert!(error("Bad.Id").contains("a-z"));
        assert!(error("empty").contains("no extension.toml"));
        assert!(error("coding-projects").contains("built-in"));

        // dbx still loads.
        config.reload().unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn several_actions_make_a_submenu_where_c_and_q_are_fine() {
        let (dir, config) = temp_config("ext-group");
        install(&config, "notes", "name = \"Notes\"\nkey = \"o\"\n[[actions]]\ntitle = \"Close\"\nkey = \"c\"\nrun = \"echo closed\"\n[[actions]]\ntitle = \"Query\"\nkey = \"q\"\nrun = \"echo hi\"\nterminal = true\n", "");
        let config = config.reload().unwrap();
        let menu: Vec<_> = crate::actions::registry(&config).iter().map(|a| (a.key(), a.title().to_string())).collect();
        assert!(menu.contains(&('o', "Notes".into())), "{menu:?}");

        let ext = &discover(&config.path, &[]).0[0];
        let ctx = Ctx { config: &config };
        assert!(matches!(ext.actions[0].run(&ctx).unwrap(), Step::Done(m) if m == "closed"));
        assert!(matches!(ext.actions[1].run(&ctx).unwrap(), Step::RunInTerminal { .. }));
        fs::remove_dir_all(&dir).unwrap();
    }
}
