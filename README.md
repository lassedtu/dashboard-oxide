# dbx

**dashboard-oxide**: a small terminal dashboard for things I do often.

- **Coding projects**
  - **New project**: name it, and dbx creates the folder in your projects directory, adds a README, runs `git init`, and opens it in your editor.
  - **Recent projects**: lists your most recently active projects and opens the one you pick.
- **Reports** (off by default): school reports in [Typst](https://typst.app).
  - **New report**: pick a course, name the report, and dbx creates `<course>/Assignments/Report-Name/` from the Typst template with title, course and author filled in, then opens it in your editor.
  - **Recent reports**: your most recently active reports across all courses.
- **Config**: view and change every setting from inside dbx, or open `~/.dbx/config.toml` in your editor.

Features come as **extensions** (Coding projects is one), each with its own settings, switched on and off under Config → Extensions. You can add your own as a folder with a small TOML file and a script; see [Installing extensions](#installing-extensions).

Built with Rust and [ratatui](https://ratatui.rs). Designed so new features are a single folder.

## Install

Requires a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
git clone https://github.com/lassedtu/dashboard-oxide && cd dbx
./install.sh
```

This builds a release binary and installs it as `dbx` in `~/.cargo/bin`. The script tells you if that folder isn't on your `PATH`, or if another `dbx` would shadow it. To install somewhere else, set `CARGO_INSTALL_ROOT` (the binary goes in `$CARGO_INSTALL_ROOT/bin`).

To update, pull and run `./install.sh` again. To remove it:

```sh
./install.sh --uninstall   # your config in ~/.dbx is kept
```

## Usage

```sh
dbx            # open the dashboard
dbx --version
dbx --help
```

| Key | Does |
|---|---|
| Highlighted letter (`p`, `c`, `q`, ...) | Run that entry; works in submenus too (`p` then `n` = new project) |
| `↑` `↓` or `j` `k` | Move |
| `Enter` | Select / submit |
| `Esc` | Back one level (quits from the main menu) |
| `Ctrl+C` | Quit from anywhere |

If an action fails (say, a project name that already exists), you stay on the same screen with your input intact, so you can fix it and press Enter again.

## Configuration

Press `c` for the Config screen:

```
Config                     Editor, Quit after opening, Extensions, Open config.toml
  └ Extensions             every extension: Enter turns it on/off, s (or →) opens its settings
      └ Coding projects    Enabled, Projects folder, Recent projects shown, ...
```

Every setting shows its current value. Pressing Enter on one:

- **On/off settings** flip.
- **Folders, numbers and text** open a prompt pre-filled with the current value. Invalid input (a folder that doesn't exist, a count of 0) is rejected and nothing is saved.
- **Editor** offers VS Code, Cursor, Zed, Sublime, Neovim, Vim, and Helix, or "Custom…" for any command. Only editors found on your `PATH` are saved.

Changes are written straight to the file, touching only the line that changed, so your comments are kept. **Open config.toml in editor** opens the file itself.

On first run dbx creates `~/.dbx/config.toml` with every setting and a comment explaining it. It starts like this:

```toml
# dbx configuration
#
# Every setting can also be changed from the Config screen inside dbx (press c).
# Settings missing from this file are added back with their defaults.

# Command used to open projects and files. May include arguments, e.g. "code -n".
# Terminal editors (nvim, vim, hx, ...) take over the terminal while open.
editor = "code"

# Quit dbx after opening something in the editor.
close_on_open = true

# Coding projects: New and recent projects
[extensions.coding-projects]
enabled = true

# Folder where new projects are created and recent projects are listed from.
projects_dir = "~/Developer/Projekter"
...
```

**The file keeps itself up to date.** Every time dbx loads it, any setting that's missing (because dbx was updated, or you installed an extension) is added with its default and comment. Keys you already have are never changed. Values of the wrong type (say, `recent_count = "ten"`) are reported when dbx starts. Config files from before extensions, with `projects_dir` and `[new_project]` at the top, are moved into `[extensions.coding-projects]` automatically. dbx re-reads the file every time you start an action, so edits take effect without restarting.

**Editors:** GUI editors (VS Code, Zed, ...) are launched in the background. Terminal editors (`nvim`, `vim`, `vi`, `nano`, `hx`, `micro`, `kak`, `emacs -nw`, ...) get the terminal: dbx steps aside while the editor runs and comes back when you quit it. When `close_on_open` is on, dbx exits and the editor opens in its place.

**Reports:** turn it on and set your name under Config → Extensions → Reports. Course folders are the folders in `root` whose name starts with a course number, so folders like `Misc` or `Old Courses` are skipped. A report named `Lab report 2` goes in `Lab-report-2/`; its `src/report-settings.typ` gets `title = "Lab report 2"`, `subtitle` = the course folder's name, and `author` = your name. Nothing else in the template is changed.

The template in `assets/templates/typst-template/` is built into `dbx`. To use a different one without rebuilding, put it in `~/.dbx/templates/typst-template/`; it needs a `src/report-settings.typ` with `#let title`, `#let subtitle` and `#let author` lines.

**How "recent" is decided:** a folder's own modified time only changes when files directly inside it are added or removed, so dbx also checks `.git/index` and `.git/HEAD`. Staging, committing, and switching branches all count as activity. Hidden folders are skipped.

## Installing extensions

An extension can also be a folder in `~/.dbx/extensions/`, with an `extension.toml` and whatever scripts it runs. No Rust, no rebuilding: dbx picks it up the next time it shows the menu, adds its settings to `config.toml`, and lists it under Config → Extensions. If its `extension.toml` has a mistake, it shows there with `[!]`, and Enter shows the error.

```toml
# ~/.dbx/extensions/todo/extension.toml   (the folder name, "todo", is the id)
name = "Todo"
description = "Append to a todo file"

[[settings]]                # optional, any number; editable under Config → Extensions → Todo
key = "file"
label = "Todo file"
kind = "text"               # text, folder, count, toggle or editor
default = "~/todo.md"
help = "File new todos are added to."

[[actions]]                 # one action goes straight in the main menu
title = "Add todo"
key = "t"
run = "./add.sh"            # run with `sh -c` in the extension's folder
prompt = "Todo"             # optional: ask for a line of text first
```

`add.sh` gets everything through environment variables:

```sh
#!/bin/sh
file="${DBX_SETTING_FILE/#\~/$HOME}"
[ -n "$DBX_INPUT" ] || { echo "nothing to add" >&2; exit 1; }
echo "- [ ] $DBX_INPUT" >> "$file"
echo "added to $file"
```

| Variable | Value |
|---|---|
| `DBX_INPUT` | What the user typed, if the action has a `prompt` |
| `DBX_SETTING_<KEY>` | Each setting, key upper-cased (`folder` settings have `~` expanded) |
| `DBX_EDITOR` | The configured editor command |
| `DBX_CONFIG` | Path to `config.toml` |
| `DBX_EXTENSION_DIR` | The extension's folder |

The last line the command prints becomes the status message. If it exits with an error, the last line of its stderr is shown in red and you stay where you were (on the prompt, input intact).

More options:

- **Several actions:** add more `[[actions]]` and a top-level `key = "o"`. The extension then gets a submenu, opened with that key.
- **Interactive programs:** `terminal = true` on an action gives it the terminal (like nvim) until it exits.
- **Off by default:** `default_enabled = false`.
- **Keys:** `j` and `k` are for moving; in the main menu `c` and `q` are taken too. If two extensions use the same key, the first one in the menu gets it.

Commands run while dbx waits, so keep them quick, or use `terminal = true` for anything long-running. An extension is just a script you installed: it can do anything your shell can, so only install ones you trust.

## Project layout

```
assets/logo.ans          ANSI-art logo, compiled into the binary
assets/templates/        report template, compiled into the binary
build.rs                 rebuilds when the template changes
install.sh               installer / uninstaller
src/
  main.rs                CLI flags, terminal setup and teardown
  app.rs                 app state, screens, key handling
  ui.rs                  all drawing
  config.rs              loading, migrating and editing ~/.dbx/config.toml;
                         the Setting type (key, label, kind, default)
  sys.rs                 shell-outs (editor detection, git, PATH lookup)
  actions/
    mod.rs               Action trait, Step enum, registry
    config_menu.rs       the Config screen, built from Setting lists
    quit.rs
  extensions/
    mod.rs               Extension struct, list of all extensions
    coding_projects/
      mod.rs             the extension: settings, actions
      new_project.rs
      recent_projects.rs
    external.rs          extensions from ~/.dbx/extensions/*/extension.toml
    reports/
      mod.rs             the extension: settings, finding courses
      new_report.rs      course picker → name → create from template
      recent_reports.rs
      template.rs        embedded template, filling report-settings.typ
    folders.rs           shared: list folders by activity, create, validate names
```

## Adding an extension

Menu entries are **actions**, grouped into **extensions**. An extension with one action puts it straight in the main menu; with several, it gets a submenu (like Coding projects). Config and Quit are core actions and always shown; everything else belongs to an extension.

Actions never draw anything. They return a `Step`, and the app turns that into a screen:

| Step | Effect |
|---|---|
| `Step::Done(msg)` | Back to the menu and show `msg` |
| `Step::Quit(msg)` | Exit, printing `msg` to the shell if given |
| `Step::prompt(label, \|text, ctx\| ...)` | Ask for a line of text |
| `Step::pick(title, items, \|index, ctx\| ...)` | Let the user choose from a list |
| `Step::RunInTerminal { command, then }` | Hand the terminal to `command` until it exits, then continue with `then` |

To open something in the user's editor, call `open_in_editor(ctx, path, then)` from `actions/mod.rs`. It picks background or terminal mode for you.

`prompt` and `pick` take a closure that returns the next `Step`, so an action with several stages is just a chain of closures. Return an `Err` (for example with `bail!`) and dbx shows it in red while the user stays on the same screen.

For more control, build a `Prompt` or `Pick` directly and convert it with `.into()`:

```rust
Prompt::new("Name", on_submit)
    .initial("current value")          // pre-fill the input
    .back(|ctx| Ok(previous_screen))   // what Esc does (default: main menu)
    .into()

Pick::new("Choose", items, on_select)
    .selected(2)                       // start on this row
    .shortcuts(vec!['a', 'b'])         // one key per item
    .back(|ctx| Ok(previous_screen))
    .into()
```

Esc from the first screen of an action in a submenu returns to that submenu.

`ctx.config` is re-read from disk before every action and callback, so it always matches the file.

1. Create `src/extensions/my_ext/my_action.rs`:

   ```rust
   use anyhow::Result;

   use crate::actions::{Action, Ctx, Step};

   pub struct MyAction;

   impl Action for MyAction {
       fn key(&self) -> char { 'm' }
       fn title(&self) -> &str { "My action" }

       fn run(&self, _ctx: &Ctx) -> Result<Step> {
           Ok(Step::prompt("What's your name?", |name, _ctx| {
               Ok(Step::Done(format!("hi {name}")))
           }))
       }
   }
   ```

2. Describe the extension in `src/extensions/my_ext/mod.rs`:

   ```rust
   mod my_action;

   use serde::Deserialize;

   use super::Extension;
   use crate::config::{Config, Kind, Setting};

   pub fn extension() -> Extension {
       Extension {
           id: "my-ext".into(),        // its table in config.toml; don't change it later
           name: "My extension".into(),
           description: "Says hi".into(),
           key: 'm',                   // submenu shortcut, if it has several actions
           default_enabled: true,
           settings: vec![
               Setting::new("greeting", "Greeting", Kind::Text, "hi"),
           ],
           actions: vec![Box::new(my_action::MyAction)],
           dir: None,              // only set for extensions installed from a folder
       }
   }

   #[derive(Deserialize)]
   struct Settings {
       greeting: String,
   }

   /// Call from actions: the user's values, with defaults filled in.
   fn settings(config: &Config) -> anyhow::Result<Settings> {
       extension().load(config)
   }
   ```

3. Add `pub mod my_ext;` and `my_ext::extension()` to `all()` in `src/extensions/mod.rs`.

It shows up in the menu, and under Config → Extensions with an editable page for its settings, straight away. Setting kinds are `Text`, `Folder`, `Count`, `Toggle` and `Editor`; the kind decides how the value is shown, validated and edited, so a setting needs no UI code. If you add the extension's table to the `DEFAULT_CONFIG` template, a test checks it matches the defaults.

**Core settings** (not owned by an extension) are fields on `Config`, listed in `core_settings()` in `src/config.rs`.

For running external commands, add a helper to `sys.rs` so output never lands on top of the TUI.

## Development

```sh
cargo run      # run from source
cargo test
```

The logo lives in `assets/logo.ans` and may contain ANSI color codes. It and the report template are embedded at compile time, so rebuild (or reinstall) after editing them.
