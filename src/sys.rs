//! Thin wrappers around external commands. Output is discarded/captured so
//! nothing scribbles over the TUI.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Editors that run inside the terminal and need it handed over to them.
const TERMINAL_EDITORS: &[&str] = &[
    "nvim", "vim", "vi", "nano", "hx", "helix", "micro", "kak", "nvi", "joe", "ne",
];

pub struct EditorCommand {
    pub command: Command,
    /// True for terminal editors: they must run in the foreground with the
    /// TUI suspended, instead of being launched in the background.
    pub in_terminal: bool,
}

/// Build the command that opens `path` in `editor`.
/// `editor` may include arguments, e.g. `"code -n"`.
pub fn editor_command(editor: &str, path: &Path) -> Result<EditorCommand> {
    let mut parts = editor.split_whitespace();
    let program = parts.next().context("`editor` in config is empty")?;
    let args: Vec<&str> = parts.collect();

    if find_on_path(program).is_none() {
        bail!("editor `{program}` not found on PATH");
    }

    let mut command = Command::new(program);
    command.args(&args).arg(path);
    Ok(EditorCommand { command, in_terminal: is_terminal_editor(program, &args) })
}

fn is_terminal_editor(program: &str, args: &[&str]) -> bool {
    let name = Path::new(program).file_name().and_then(|n| n.to_str()).unwrap_or(program);
    // `emacs -nw` runs in the terminal; plain `emacs` opens a window.
    TERMINAL_EDITORS.contains(&name) || (name == "emacs" && args.contains(&"-nw"))
}

/// Launch a GUI program without waiting for it.
pub fn spawn_detached(mut command: Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to run `{program}`"))?;
    Ok(())
}

/// Open a URL (e.g. `obsidian://...`) with the system's handler for it.
pub fn open_url(url: &str) -> Result<()> {
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let mut command = Command::new(opener);
    command.arg(url);
    spawn_detached(command)
}

/// Run a program with no terminal and wait, returning its stdout. On failure
/// the error is the last line it printed to stderr (or stdout).
pub fn run_captured(mut command: Command) -> Result<String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to run `{program}`"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let last = |text: &str| text.lines().rev().map(str::trim).find(|l| !l.is_empty()).map(String::from);
    match last(&stderr).or_else(|| last(&stdout)) {
        Some(line) => bail!("{line}"),
        None => bail!("`{program}` failed ({})", output.status),
    }
}

/// Run a program in the foreground with the terminal attached, and wait.
/// The caller is responsible for suspending the TUI around this.
pub fn run_foreground(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command.status().with_context(|| format!("failed to run `{program}`"))?;
    if !status.success() {
        bail!("`{program}` exited with {status}");
    }
    Ok(())
}

/// Resolve a program name like `which` does. Paths containing `/` are checked directly.
pub fn find_on_path(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let path = PathBuf::from(program);
        return path.is_file().then_some(path);
    }
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

pub fn git_init(path: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("init")
        .current_dir(path)
        .stdin(Stdio::null())
        .output()
        .context("failed to run `git` (is it installed?)")?;

    if !output.status.success() {
        bail!("git init failed: {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_terminal_editors() {
        assert!(is_terminal_editor("nvim", &[]));
        assert!(is_terminal_editor("/opt/homebrew/bin/hx", &[]));
        assert!(is_terminal_editor("emacs", &["-nw"]));
        assert!(!is_terminal_editor("emacs", &[]));
        assert!(!is_terminal_editor("code", &["-n"]));
    }

    #[test]
    fn finds_programs_on_path() {
        assert!(find_on_path("sh").is_some());
        assert!(find_on_path("/bin/sh").is_some());
        assert!(find_on_path("definitely-not-a-real-program-dbx").is_none());
    }
}
