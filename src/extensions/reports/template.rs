//! The Typst report template: copied into each new report, then
//! `src/report-settings.typ` gets the report's title, course and author.
//!
//! The template in `assets/templates/typst-template` is built into the binary.
//! A copy in `~/.dbx/templates/typst-template` is used instead if present,
//! so it can be changed without rebuilding dbx.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use include_dir::{Dir, include_dir};

use crate::config;

static EMBEDDED: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/templates/typst-template");

const SETTINGS_FILE: &str = "src/report-settings.typ";

pub struct Fields<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub author: &'a str,
}

/// Copy the template into `dest` (an existing, empty folder) and fill in its settings.
pub fn write(dest: &Path, fields: &Fields) -> Result<()> {
    match override_dir()? {
        Some(dir) => copy_dir(&dir, dest)?,
        None => EMBEDDED
            .extract(dest)
            .with_context(|| format!("copying the template to {}", dest.display()))?,
    }

    let path = dest.join(SETTINGS_FILE);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("the template has no {SETTINGS_FILE}"))?;
    fs::write(&path, fill(&text, fields)?).with_context(|| format!("writing {}", path.display()))
}

fn override_dir() -> Result<Option<PathBuf>> {
    let dir = config::config_dir()?.join("templates/typst-template");
    Ok(dir.is_dir().then_some(dir))
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    for entry in fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)
                .with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Replace the values of `#let title = ...`, `#let subtitle = ...` and
/// `#let author = ...`, leaving everything else as it is.
fn fill(text: &str, fields: &Fields) -> Result<String> {
    let values = [("title", fields.title), ("subtitle", fields.subtitle), ("author", fields.author)];
    let mut found = [false; 3];

    let mut out: Vec<String> = text
        .lines()
        .map(|line| {
            for (i, (key, value)) in values.iter().enumerate() {
                let rest = line.trim_start().strip_prefix("#let ").map(str::trim_start);
                let is_key = rest
                    .and_then(|r| r.strip_prefix(key))
                    .is_some_and(|r| r.trim_start().starts_with('='));
                if is_key {
                    found[i] = true;
                    return format!("#let {key} = {}", typst_string(value));
                }
            }
            line.to_string()
        })
        .collect();

    if let Some(i) = found.iter().position(|f| !f) {
        bail!("{SETTINGS_FILE} in the template has no `#let {} = ...` line", values[i].0);
    }
    if text.ends_with('\n') {
        out.push(String::new());
    }
    Ok(out.join("\n"))
}

fn typst_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELDS: Fields = Fields { title: "Lab \"2\"", subtitle: "02323 Stats", author: "A. B" };

    #[test]
    fn fills_only_the_three_values() {
        let text = "// Metadata\n#let title = \"Title\"\n#let subtitle = \"S\"\n#let author=\"Author\"\n#let titles = 1\n";
        let filled = fill(text, &FIELDS).unwrap();
        assert_eq!(
            filled,
            "// Metadata\n#let title = \"Lab \\\"2\\\"\"\n#let subtitle = \"02323 Stats\"\n#let author = \"A. B\"\n#let titles = 1\n"
        );
        assert!(fill("#let title = \"x\"\n", &FIELDS).is_err());
    }

    #[test]
    fn embedded_template_has_fillable_settings() {
        let file = EMBEDDED.get_file(SETTINGS_FILE).expect("settings file in template");
        fill(file.contents_utf8().unwrap(), &FIELDS).unwrap();
        assert!(EMBEDDED.get_file("main.typ").is_some());
    }
}
