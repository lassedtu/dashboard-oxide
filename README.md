# dbx

**dashboard-oxide**: a small terminal dashboard for things I do often.

- **New project**: name it, and dbx creates the folder in your projects directory, adds a README, runs `git init`, and opens it in your editor.
- **Recent projects**: lists your most recently active projects and opens the one you pick.
- **Config**: view and change every setting from inside dbx, or open `~/.dbx/config.toml` in your editor.

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
| `n` / `r` / `c` / `q` | Run the action with that shortcut |
| `↑` `↓` or `j` `k` | Move |
| `Enter` | Select / submit |
| `Esc` | Back one level (quits from the main menu) |
| `Ctrl+C` | Quit from anywhere |

If an action fails (say, a project name that already exists), you stay on the same screen with your input intact, so you can fix it and press Enter again.

## Configuration

Press `c` for the Config screen. It lists every setting with its current value:

- **On/off settings** flip when you press Enter.
- **Projects folder and recent count** open a prompt pre-filled with the current value. Invalid input (a folder that doesn't exist, a count of 0) is rejected and nothing is saved.
- **Editor** offers VS Code, Cursor, Zed, Sublime, Neovim, Vim, and Helix, or "Custom…" for any command. Only editors found on your `PATH` are saved.
- **Open config.toml in editor** opens the file itself.

Changes are written straight to the file, touching only the line that changed, so your comments are kept.

On first run dbx creates `~/.dbx/config.toml`:

```toml
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
```

Any key you leave out falls back to its default, so old config files keep working as new options are added. dbx re-reads the file every time you start an action, so edits take effect without restarting.

**Editors:** GUI editors (VS Code, Zed, ...) are launched in the background. Terminal editors (`nvim`, `vim`, `vi`, `nano`, `hx`, `micro`, `kak`, `emacs -nw`, ...) get the terminal: dbx steps aside while the editor runs and comes back when you quit it. When `close_on_open` is on, dbx exits and the editor opens in its place.

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
  sys.rs                 shell-outs (editor detection, git, PATH lookup)
  actions/
    mod.rs               Action trait, Step enum, registry
    new_project.rs
    recent_projects.rs
    config_menu.rs       the Config screen and its settings
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
    .back(|ctx| Ok(previous_screen))
    .into()
```

`ctx.config` is re-read from disk before every action and callback, so it always matches the file.

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

**Adding a setting:**
1. Add a field to `Config` in `src/config.rs`.
2. Add it to `Default` and to the `DEFAULT_CONFIG` template. A test fails if those two drift apart.
3. To make it editable from the Config screen, add a variant to `Setting` in `src/actions/config_menu.rs`, list it in `SETTINGS`, and fill in the `match`es.

For running external commands, add a helper to `sys.rs` so output never lands on top of the TUI.

## Development

```sh
cargo run      # run from source
cargo test
```

The logo lives in `assets/logo.ans` and may contain ANSI color codes. It's embedded at compile time, so rebuild after editing it.
