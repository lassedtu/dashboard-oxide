//! `dbx ext ...`: managing extensions from the shell.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::config::{self, Config};
use crate::extensions::{self, external};

pub const EXT_HELP: &str = "\
Usage: dbx ext <command>

Commands:
  list         Show all extensions, whether they're on, and any that failed to load
  new <id>     Create a starter extension in ~/.dbx/extensions/<id>/";

pub fn ext(args: &[String]) -> Result<()> {
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["list"] => list(&Config::load()?),
        ["new", id] => {
            let config = Config::load()?;
            let dir = new(&config.path, id)?;
            println!("Created {}", config::tilde(&dir));
            println!("Edit extension.toml and run.sh there; it's already in the dbx menu.");
            Ok(())
        }
        [] | ["help" | "-h" | "--help"] => {
            println!("{EXT_HELP}");
            Ok(())
        }
        _ => bail!("unknown command `dbx ext {}`\n\n{EXT_HELP}", args.join(" ")),
    }
}

fn list(config: &Config) -> Result<()> {
    for ext in extensions::all(config) {
        let state = if ext.is_enabled(config) { "on " } else { "off" };
        let source = ext.dir.as_deref().map_or("built-in".into(), config::tilde);
        println!("{state}  {:<18} {:<20} {source}", ext.id, ext.name);
    }
    for broken in extensions::broken(config) {
        println!("!    {:<18} {}", broken.id, broken.error);
    }
    Ok(())
}

const STARTER_MANIFEST: &str = r#"name = "{name}"
description = "What it does, in a few words"

# Settings appear under Config -> Extensions -> {name} and reach run.sh as
# DBX_SETTING_<KEY>. Kinds: text, folder, count, toggle, editor.
[[settings]]
key = "greeting"
label = "Greeting"
kind = "text"
default = "Hello"
help = "What to greet with."

# One action goes straight in the main menu. For several, add more
# [[actions]] and a top-level `key = "x"` for the submenu.
[[actions]]
title = "{name}"
key = "{key}"
run = "./run.sh"
prompt = "Your name"    # optional; the answer is $DBX_INPUT
# terminal = true       # give the command the terminal (interactive programs)
"#;

const STARTER_SCRIPT: &str = r#"#!/bin/sh
# Runs in this folder. The last line printed becomes dbx's status message;
# exit non-zero and the last line on stderr is shown as an error.
[ -n "$DBX_INPUT" ] || { echo "type a name first" >&2; exit 1; }
echo "$DBX_SETTING_GREETING, $DBX_INPUT!"
"#;

/// Create a working starter extension in the extensions folder.
fn new(config_path: &Path, id: &str) -> Result<PathBuf> {
    let root = external::dir_for(config_path).context("no extensions folder")?;
    let dir = root.join(id);
    if dir.exists() {
        bail!("{} already exists", config::tilde(&dir));
    }

    let name = id
        .split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| w[..1].to_uppercase() + &w[1..])
        .collect::<Vec<_>>()
        .join(" ");
    // Pick a key that's free in the main menu.
    let taken: Vec<char> = crate::actions::registry(&Config::load_from(config_path)?)
        .iter()
        .map(|a| a.key())
        .collect();
    let key = id
        .chars()
        .chain('a'..='z')
        .find(|c| c.is_ascii_lowercase() && !taken.contains(c) && !"jkcq".contains(*c))
        .unwrap_or('x');

    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let manifest = STARTER_MANIFEST.replace("{name}", &name).replace("{key}", &key.to_string());
    fs::write(dir.join(external::MANIFEST), manifest)?;
    let script = dir.join("run.sh");
    fs::write(&script, STARTER_SCRIPT)?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;

    // Check it loads, so a bad id is caught here rather than shown as broken.
    let builtin: Vec<String> = Vec::new();
    let (_, broken) = external::discover(config_path, &builtin);
    if let Some(b) = broken.into_iter().find(|b| b.id == id) {
        fs::remove_dir_all(&dir)?;
        bail!("{}", b.error);
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{Ctx, Step};
    use crate::config::tests::temp_config;

    #[test]
    fn new_creates_a_working_extension_with_a_free_key() {
        let (dir, config) = temp_config("cli-new");
        let ext_dir = new(&config.path, "my-tool").unwrap();
        assert!(new(&config.path, "my-tool").is_err());
        assert!(new(&config.path, "Bad.Id").is_err());
        assert!(!ext_dir.parent().unwrap().join("Bad.Id").exists());

        let config = config.reload().unwrap();
        let ext = extensions::all(&config).into_iter().find(|e| e.id == "my-tool").unwrap();
        assert_eq!(ext.name, "My Tool");
        assert_eq!(ext.key, 'm');
        let ctx = Ctx { config: &config };
        let Step::Prompt(mut prompt) = ext.actions[0].run(&ctx).unwrap() else { panic!() };
        let Step::Done(message) = (prompt.on_submit)("Ada".into(), &ctx).unwrap() else { panic!() };
        assert_eq!(message, "Hello, Ada!");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn example_extension_in_repo_works() {
        let (dir, config) = temp_config("cli-example");
        let target = external::dir_for(&config.path).unwrap().join("todo");
        fs::create_dir_all(&target).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/extensions/todo");
        for file in ["extension.toml", "add.sh"] {
            fs::copy(example.join(file), target.join(file)).unwrap();
        }
        let todo_file = dir.join("todo.md");
        config::set(&config.path, "extensions.todo.file", todo_file.display().to_string()).unwrap();
        let config = config.reload().unwrap();

        let ext = extensions::all(&config).into_iter().find(|e| e.id == "todo").expect("loads");
        let ctx = Ctx { config: &config };
        let Step::Prompt(mut prompt) = ext.actions[0].run(&ctx).unwrap() else { panic!() };
        assert!((prompt.on_submit)("".into(), &ctx).is_err());
        assert!((prompt.on_submit)("write tests".into(), &ctx).is_ok());
        assert_eq!(fs::read_to_string(&todo_file).unwrap(), "- [ ] write tests\n");

        fs::remove_dir_all(&dir).unwrap();
    }
}
