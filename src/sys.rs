//! Thin wrappers around external commands. Output is discarded/captured so
//! nothing scribbles over the TUI.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Launch the editor on `path` without waiting for it.
/// `editor` may include arguments, e.g. `"code -n"`.
pub fn open_in_editor(editor: &str, path: &Path) -> Result<()> {
    let mut parts = editor.split_whitespace();
    let program = parts.next().context("`editor` in config is empty")?;

    Command::new(program)
        .args(parts)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to run `{program}` (is it on your PATH?)"))?;
    Ok(())
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
