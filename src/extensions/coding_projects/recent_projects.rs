use std::time::SystemTime;

use anyhow::Result;

use super::settings;
use crate::actions::{Action, Ctx, Step, open_project};
use crate::extensions::folders::{self, Folder};

pub struct RecentProjects;

impl Action for RecentProjects {
    fn key(&self) -> char {
        'r'
    }

    fn title(&self) -> &str {
        "Recent projects"
    }

    fn run(&self, ctx: &Ctx) -> Result<Step> {
        let settings = settings(ctx.config)?;
        let dir = &settings.projects_dir;
        let mut projects = folders::subfolders(dir)?;
        folders::newest_first(&mut projects, settings.recent_count);
        if projects.is_empty() {
            return Ok(Step::Done(format!("no projects in {}", dir.display())));
        }

        let now = SystemTime::now();
        let items = projects.iter().map(|p| format_item(p, now)).collect();
        Ok(Step::pick("Recent projects", items, move |i, ctx| {
            let project = &projects[i];
            open_project(ctx, &project.path, &format!("opened {}", project.name))
        }))
    }
}

fn format_item(project: &Folder, now: SystemTime) -> String {
    const NAME_WIDTH: usize = 36;
    let name = folders::truncate(&project.name, NAME_WIDTH);
    format!("{name:<NAME_WIDTH$}{:>9}", folders::humanize_age(project.last_activity, now))
}
