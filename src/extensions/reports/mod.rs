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
        dir: None,
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

fn courses(settings: &Settings) -> Result<Vec<Folder>> {
    folders::courses(&settings.root)
}
