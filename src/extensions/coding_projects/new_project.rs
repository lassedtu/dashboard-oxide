use std::fs;

use anyhow::{Context, Result};

use super::settings;
use crate::actions::{Action, Ctx, Step, open_project};
use crate::extensions::folders;
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
    let name = folders::validate_name(&name)?;
    let settings = settings(ctx.config)?;
    let path = settings.projects_dir.join(name);
    folders::create_new(&path)?;

    if settings.create_readme {
        fs::write(path.join("README.md"), format!("# {name}\n")).context("writing README.md")?;
    }
    if settings.git_init {
        sys::git_init(&path)?;
    }

    open_project(ctx, &path, &format!("created {name}"))
}
