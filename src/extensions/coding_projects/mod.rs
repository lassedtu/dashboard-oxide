//! Coding projects: create a new project folder, or open a recent one.

mod new_project;
mod recent_projects;

use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

use super::Extension;
use crate::config::{self, Config, Kind, Setting};

pub const ID: &str = "coding-projects";

pub fn extension() -> Extension {
    Extension {
        id: ID.into(),
        name: "Coding projects".into(),
        description: "New and recent projects".into(),
        key: 'p',
        default_enabled: true,
        settings: vec![
            Setting::new("projects_dir", "Projects folder", Kind::Folder, "~/Developer/Projekter"),
            Setting::new("recent_count", "Recent projects shown", Kind::Count, 10),
            Setting::new("git_init", "git init new projects", Kind::Toggle, true),
            Setting::new("create_readme", "README in new projects", Kind::Toggle, true),
        ],
        actions: vec![
            Box::new(new_project::NewProject),
            Box::new(recent_projects::RecentProjects),
        ],
    }
}

/// This extension's settings, with defaults filled in and `~` expanded.
#[derive(Deserialize)]
struct Settings {
    projects_dir: PathBuf,
    recent_count: usize,
    git_init: bool,
    create_readme: bool,
}

fn settings(config: &Config) -> Result<Settings> {
    let mut settings: Settings = extension().load(config)?;
    settings.projects_dir = config::expand_tilde(&settings.projects_dir)?;
    Ok(settings)
}
