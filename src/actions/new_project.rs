use std::fs;

use anyhow::{Context, Result, bail};

use super::{Action, Ctx, Step, open_project};
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
    let config = ctx.config;

    fs::create_dir_all(&config.projects_dir)
        .with_context(|| format!("creating {}", config.projects_dir.display()))?;

    let path = config.projects_dir.join(name);
    // create_dir (not _all) fails if it already exists, so we never clobber a project.
    fs::create_dir(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => anyhow::anyhow!("{name} already exists"),
        _ => anyhow::Error::new(e).context(format!("creating {}", path.display())),
    })?;

    if config.new_project.create_readme {
        fs::write(path.join("README.md"), format!("# {name}\n")).context("writing README.md")?;
    }
    if config.new_project.git_init {
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
