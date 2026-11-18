use std::fs;

use anyhow::{Context, Result, bail};

use super::settings;
use crate::actions::{Action, Ctx, Step, open_project};
use crate::sys;

pub struct NewProject;

impl Action for NewProject {
    fn key(&self) -> char {
        'n'
    }

    fn title(&self) -> &str {
        "New project"
    }

    fn run(&self, _ctx: &Ctx) -> Result<Step> {
        Ok(Step::prompt("New project name", create))
    }
}

fn create(name: String, ctx: &Ctx) -> Result<Step> {
    let name = validate_name(&name)?;
    let settings = settings(ctx.config)?;
    let dir = &settings.projects_dir;

    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let path = dir.join(name);
    // create_dir (not _all) fails if it already exists, so we never clobber a project.
    fs::create_dir(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => anyhow::anyhow!("{name} already exists"),
        _ => anyhow::Error::new(e).context(format!("creating {}", path.display())),
    })?;

    if settings.create_readme {
        fs::write(path.join("README.md"), format!("# {name}\n")).context("writing README.md")?;
    }
    if settings.git_init {
        sys::git_init(&path)?;
    }

    open_project(ctx, &path, &format!("created {name}"))
}

fn validate_name(raw: &str) -> Result<&str> {
    let name = raw.trim();
    if name.is_empty() {
        bail!("name can't be empty");
    }
    if name.contains(['/', '\\']) {
        bail!("name can't contain slashes");
    }
    if name.starts_with('.') {
        bail!("name can't start with a dot");
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_names() {
        assert_eq!(validate_name("  my-app ").unwrap(), "my-app");
        assert!(validate_name("   ").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("..").is_err());
        assert!(validate_name(".hidden").is_err());
    }
}
