use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};

use super::{Action, Ctx, Step, open_project};

pub struct RecentProjects;

impl Action for RecentProjects {
    fn key(&self) -> char {
        'r'
    }

    fn title(&self) -> &str {
        "Recent projects"
    }

    fn run(&self, ctx: &Ctx) -> Result<Step> {
        let dir = &ctx.config.projects_dir;
        let projects = list_recent(dir, ctx.config.recent_count)?;
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

struct Project {
    name: String,
    path: PathBuf,
    last_activity: SystemTime,
}

/// Non-hidden folders in `dir`, most recently active first.
fn list_recent(dir: &Path, limit: usize) -> Result<Vec<Project>> {
    let entries = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;

    let mut projects: Vec<Project> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if name.starts_with('.') {
                return None;
            }
            let path = e.path();
            let last_activity = last_activity(&path)?;
            Some(Project { name, path, last_activity })
        })
        .collect();

    projects.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    projects.truncate(limit);
    Ok(projects)
}

/// A folder's own mtime only changes when direct children are added/removed,
/// so also look at git's index and HEAD, which change on staging/commits/checkouts.
fn last_activity(path: &Path) -> Option<SystemTime> {
    [path.to_path_buf(), path.join(".git/index"), path.join(".git/HEAD")]
        .iter()
        .filter_map(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
        .max()
}

fn format_item(project: &Project, now: SystemTime) -> String {
    const NAME_WIDTH: usize = 36;
    let mut name = project.name.clone();
    if name.chars().count() > NAME_WIDTH {
        name = name.chars().take(NAME_WIDTH - 1).collect::<String>() + "…";
    }
    format!("{name:<NAME_WIDTH$}{:>9}", humanize_age(project.last_activity, now))
}

fn humanize_age(t: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(t).unwrap_or_default().as_secs();
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dbx-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sorts_by_git_activity_and_skips_hidden() {
        let dir = temp_dir("recent");
        for name in ["old", "active", ".hidden"] {
            fs::create_dir(dir.join(name)).unwrap();
        }
        // Simulate a recent commit in "active".
        fs::create_dir(dir.join("active/.git")).unwrap();
        let index = fs::File::create(dir.join("active/.git/index")).unwrap();
        index.set_modified(SystemTime::now() + Duration::from_secs(3600)).unwrap();
        fs::write(dir.join("not-a-dir.txt"), "").unwrap();

        let names: Vec<_> = list_recent(&dir, 10).unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["active", "old"]);

        assert_eq!(list_recent(&dir, 1).unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn humanizes_ages() {
        let now = SystemTime::now();
        let ago = |s| now - Duration::from_secs(s);
        assert_eq!(humanize_age(ago(5), now), "just now");
        assert_eq!(humanize_age(ago(120), now), "2m ago");
        assert_eq!(humanize_age(ago(7200), now), "2h ago");
        assert_eq!(humanize_age(ago(3 * 86400), now), "3d ago");
    }
}
