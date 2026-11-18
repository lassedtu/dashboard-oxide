# dbx

**dashboard-oxide**: a small terminal dashboard for things I do often.

- **New project**: name it, and dbx creates the folder in your projects directory, adds a README, runs `git init`, and opens it in your editor.
- **Recent projects**: lists your most recently active projects and opens the one you pick.

Built with Rust and [ratatui](https://ratatui.rs). Designed so new actions are a single file.

## Install

Requires a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
git clone <this repo> && cd dbx
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
| `n` / `r` / `q` | Run the action with that shortcut |
| `↑` `↓` or `j` `k` | Move |
| `Enter` | Select / submit |
| `Esc` | Back (quits from the main menu) |
| `Ctrl+C` | Quit from anywhere |

If an action fails (say, a project name that already exists), you stay on the same screen with your input intact, so you can fix it and press Enter again.

## Configuration

On first run dbx creates `~/.dbx/config.toml`:

```toml
# Folder where new projects are created and recent projects are listed from.
projects_dir = "~/Developer/Projekter"

# Command used to open a project folder. May include arguments, e.g. "code -n".
editor = "code"

# How many projects to show under "Recent projects".
recent_count = 10

# Quit dbx after opening a project in the editor.
close_on_open = true

[new_project]
git_init = true
create_readme = true
```

Any key you leave out falls back to its default, so old config files keep working as new options are added.

**How "recent" is decided:** a folder's own modified time only changes when files directly inside it are added or removed, so dbx also checks `.git/index` and `.git/HEAD`. Staging, committing, and switching branches all count as activity. Hidden folders are skipped.

## Project layout

```
assets/logo.ans          ANSI-art logo, compiled into the binary
install.sh               installer / uninstaller
src/
  main.rs                CLI flags, terminal setup and teardown
  app.rs                 app state, screens, key handling
  ui.rs                  all drawing
  config.rs              loading and creating ~/.dbx/config.toml
  sys.rs                 shell-outs (editor, git)
  actions/
    mod.rs               Action trait, Step enum, registry
    new_project.rs
    recent_projects.rs
    quit.rs
```

## Adding an action

Actions never draw anything. They return a `Step`, and the app turns that into a screen:

| Step | Effect |
|---|---|
| `Step::Done(msg)` | Back to the menu and show `msg` |
| `Step::Quit(msg)` | Exit, printing `msg` to the shell if given |
| `Step::prompt(label, \|text, ctx\| ...)` | Ask for a line of text |
| `Step::pick(title, items, \|index, ctx\| ...)` | Let the user choose from a list |

`prompt` and `pick` take a closure that returns the next `Step`, so an action with several stages is just a chain of closures. Return an `Err` (for example with `bail!`) and dbx shows it in red while the user stays on the same screen.

1. Create `src/actions/my_action.rs`:

   ```rust
   use anyhow::Result;

   use super::{Action, Ctx, Step};

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

2. Register it in `src/actions/mod.rs`:

   ```rust
   mod my_action;

   pub fn registry() -> Vec<Box<dyn Action>> {
       vec![
           // ...
           Box::new(my_action::MyAction),
       ]
   }
   ```

`ctx.config` gives you the loaded config. To add a setting, add a field to `Config` in `src/config.rs`, add it to `Default` and to the `DEFAULT_CONFIG` template; a test fails if those two drift apart. For running external commands, add a helper to `sys.rs` so output never lands on top of the TUI.

## Development

```sh
cargo run      # run from source
cargo test
```

The logo lives in `assets/logo.ans` and may contain ANSI color codes. It's embedded at compile time, so rebuild after editing it.
