use std::time::SystemTime;

use anyhow::Result;

use super::{courses, settings};
use crate::actions::{Action, Ctx, Step, open_project};
use crate::extensions::folders::{self, Folder, course_number};

pub struct RecentReports;

impl Action for RecentReports {
    fn key(&self) -> char {
        'r'
    }

    fn title(&self) -> &str {
        "Recent reports"
    }

    fn run(&self, ctx: &Ctx) -> Result<Step> {
        let settings = settings(ctx.config)?;
        let mut reports: Vec<(Folder, String)> = Vec::new();
        for course in courses(&settings)? {
            let dir = course.path.join(&settings.assignments_dir);
            if dir.is_dir() {
                let number = course_number(&course.name).to_string();
                reports.extend(folders::subfolders(&dir)?.into_iter().map(|f| (f, number.clone())));
            }
        }
        reports.sort_by_key(|(f, _)| std::cmp::Reverse(f.last_activity));
        reports.truncate(settings.recent_count);
        if reports.is_empty() {
            return Ok(Step::Done("no reports yet".into()));
        }

        let now = SystemTime::now();
        let items = reports.iter().map(|(f, course)| format_item(f, course, now)).collect();
        Ok(Step::pick("Recent reports", items, move |i, ctx| {
            let (report, course) = &reports[i];
            open_project(ctx, &report.path, &format!("opened {} ({course})", report.name))
        }))
    }
}

fn format_item(report: &Folder, course: &str, now: SystemTime) -> String {
    let name = folders::truncate(&report.name, 27);
    let age = folders::humanize_age(report.last_activity, now);
    format!("{name:<28}{course:<10}{age:>9}")
}
