//! Reports: school reports and assignments, written in Typst.
//!
//! Expects one folder per course in `root`, named with the course number
//! first ("02323 Introduction to Statistics"). Reports live in each course's
//! `assignments_dir`, one folder each, made from the Typst template.

mod new_report;
mod recent_reports;
mod template;

use std::path::PathBuf;

use anyhow::{Result, bail};
use serde::Deserialize;

use super::Extension;
use super::folders::{self, Folder};
use crate::config::{self, Config, Kind, Setting};

pub const ID: &str = "reports";

pub fn extension() -> Extension {
    Extension {
        id: ID.into(),
        name: "Reports".into(),
        description: "School reports in Typst".into(),
        key: 'r',
        // Built around one folder layout, so opt-in.
        default_enabled: false,
        settings: vec![
            Setting::new("name", "Your name", Kind::Text, "")
                .help("Author on new reports."),
            Setting::new("root", "Courses folder", Kind::Folder, "~/Documents/DTU")
                .help("One folder per course, named like \"02323 Introduction to Statistics\"."),
            Setting::new("assignments_dir", "Reports subfolder", Kind::Text, "Assignments")
                .help("Folder inside each course where reports are created."),
            Setting::new("recent_count", "Recent reports shown", Kind::Count, 10)
                .help("How many reports to show under \"Recent reports\"."),
        ],
        actions: vec![
            Box::new(new_report::NewReport),
            Box::new(recent_reports::RecentReports),
        ],
    }
}

#[derive(Deserialize)]
struct Settings {
    name: String,
    root: PathBuf,
    assignments_dir: String,
    recent_count: usize,
}

fn settings(config: &Config) -> Result<Settings> {
    let mut settings: Settings = extension().load(config)?;
    settings.root = config::expand_tilde(&settings.root)?;
    if !settings.root.is_dir() {
        bail!(
            "{} doesn't exist; set the courses folder under Config → Extensions → Reports",
            config::tilde(&settings.root)
        );
    }
    Ok(settings)
}

/// Course folders in `root`, by name. A course folder starts with its course
/// number, so "Misc", "Old Courses" and the like are left out.
fn courses(settings: &Settings) -> Result<Vec<Folder>> {
    let mut courses: Vec<Folder> = folders::subfolders(&settings.root)?
        .into_iter()
        .filter(|f| is_course(&f.name))
        .collect();
    courses.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(courses)
}

fn is_course(name: &str) -> bool {
    let number = name.split_whitespace().next().unwrap_or_default();
    !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn course_folders_start_with_a_number() {
        assert!(is_course("02323 Introduction to Statistics"));
        assert!(is_course("62550 UX"));
        assert!(!is_course("Old Courses"));
        assert!(!is_course("Misc"));
        assert!(!is_course("2023-notes"));
    }
}
