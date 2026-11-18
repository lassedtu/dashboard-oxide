//! Notes: open a course's notes in Obsidian (or another notes app).
//!
//! Each course folder in `root` has its notes in a folder named after the
//! course plus " Notes", e.g.
//! `~/Documents/DTU/02335 Operating systems/02335 Operating systems Notes/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::Extension;
use super::folders::{self, course_number};
use crate::actions::{Action, Ctx, Step, open_project};
use crate::config::{self, Config, Kind, Setting};
use crate::sys;

pub const ID: &str = "notes";

pub fn extension() -> Extension {
    Extension {
        id: ID.into(),
        name: "Notes".into(),
        description: "Course notes in Obsidian".into(),
        key: 'n',
        // Built around one folder layout, so opt-in.
        default_enabled: false,
        settings: vec![
            Setting::new("root", "Courses folder", Kind::Folder, "~/Documents/DTU")
                .help("One folder per course; notes are in \"<course>/<course> Notes\"."),
            Setting::new("app", "Notes app", Kind::Text, "obsidian").help(
                "\"obsidian\" opens notes in Obsidian. Anything else is run as a command\n\
                 with the notes folder, like the editor setting.",
            ),
        ],
        actions: vec![Box::new(Notes)],
        dir: None,
    }
}

#[derive(Deserialize)]
struct Settings {
    root: PathBuf,
    app: String,
}

fn settings(config: &Config) -> Result<Settings> {
    let mut settings: Settings = extension().load(config)?;
    settings.root = config::expand_tilde(&settings.root)?;
    if !settings.root.is_dir() {
        bail!(
            "{} doesn't exist; set the courses folder under Config → Extensions → Notes",
            config::tilde(&settings.root)
        );
    }
    Ok(settings)
}

struct Notes;

impl Action for Notes {
    fn key(&self) -> char {
        'n'
    }

    fn title(&self) -> &str {
        "Notes"
    }

    fn run(&self, ctx: &Ctx) -> Result<Step> {
        let settings = settings(ctx.config)?;
        let courses = course_notes(&settings.root)?;
        if courses.is_empty() {
            return Ok(Step::Done(format!("no course folders in {}", config::tilde(&settings.root))));
        }
        let now = SystemTime::now();
        let items = courses.iter().map(|c| format_item(c, now)).collect();
        Ok(Step::pick("Notes", items, move |i, ctx| {
            let course = &courses[i];
            // A course without notes yet gets an empty folder to start in.
            fs::create_dir_all(&course.notes)
                .with_context(|| format!("creating {}", course.notes.display()))?;
            open_notes(ctx, &course.notes, &format!("opened {} notes", course_number(&course.name)))
        }))
    }
}

struct CourseNotes {
    name: String,
    notes: PathBuf,
    count: usize,
    /// Newest note's modified time; `None` if there are no notes.
    last_edit: Option<SystemTime>,
}

/// Every course with its notes folder, most recently edited first; courses
/// without notes go last, by name.
fn course_notes(root: &Path) -> Result<Vec<CourseNotes>> {
    let mut list: Vec<CourseNotes> = folders::courses(root)?
        .into_iter()
        .map(|course| {
            let notes = course.path.join(format!("{} Notes", course.name));
            let mut count = 0;
            let mut last_edit = None;
            scan_notes(&notes, &mut count, &mut last_edit);
            CourseNotes { name: course.name, notes, count, last_edit }
        })
        .collect();
    list.sort_by(|a, b| b.last_edit.cmp(&a.last_edit).then_with(|| a.name.cmp(&b.name)));
    Ok(list)
}

/// Count Markdown notes under `dir` (skipping hidden folders like .obsidian).
fn scan_notes(dir: &Path, count: &mut usize, last_edit: &mut Option<SystemTime>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            scan_notes(&path, count, last_edit);
        } else if name.ends_with(".md") {
            *count += 1;
            let modified = entry.metadata().and_then(|m| m.modified()).ok();
            *last_edit = (*last_edit).max(modified);
        }
    }
}

fn format_item(course: &CourseNotes, now: SystemTime) -> String {
    let name = folders::truncate(&course.name, 25);
    let (count, age) = match course.last_edit {
        Some(t) => {
            let s = if course.count == 1 { "" } else { "s" };
            (format!("{} note{s}", course.count), folders::humanize_age(t, now))
        }
        None => ("no notes".into(), String::new()),
    };
    format!("{name:<26}{count:>11}{age:>10}")
}

/// Open a notes folder in the configured app, then quit or report back like
/// opening a project does.
fn open_notes(ctx: &Ctx, path: &Path, message: &str) -> Result<Step> {
    let settings = settings(ctx.config)?;
    if settings.app.trim().eq_ignore_ascii_case("obsidian") {
        sys::open_url(&obsidian_url(path))?;
        let message = message.to_string();
        return Ok(if ctx.config.close_on_open { Step::Quit(Some(message)) } else { Step::Done(message) });
    }
    // Any other app is opened the way the editor is (in the background, or
    // given the terminal if it's a terminal program).
    let editor_config = Config { editor: settings.app.clone(), ..ctx.config.clone() };
    open_project(&Ctx { config: &editor_config }, path, message)
}

/// `obsidian://open?path=...` opens the vault containing `path`.
fn obsidian_url(path: &Path) -> String {
    let encoded: String = path
        .to_string_lossy()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("obsidian://open?path={encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::folders::tests::temp_dir;

    #[test]
    fn lists_courses_with_note_counts_newest_first() {
        let root = temp_dir("notes");
        for course in ["02335 Operating systems", "02323 Statistics", "Old Courses"] {
            fs::create_dir(root.join(course)).unwrap();
        }
        let os_notes = root.join("02335 Operating systems/02335 Operating systems Notes");
        fs::create_dir_all(os_notes.join("week 1")).unwrap();
        fs::create_dir_all(os_notes.join(".obsidian")).unwrap();
        fs::write(os_notes.join("intro.md"), "").unwrap();
        fs::write(os_notes.join("week 1/threads.md"), "").unwrap();
        fs::write(os_notes.join(".obsidian/workspace.md"), "").unwrap();
        fs::write(os_notes.join("diagram.png"), "").unwrap();

        let list = course_notes(&root).unwrap();
        let names: Vec<_> = list.iter().map(|c| (c.name.as_str(), c.count)).collect();
        assert_eq!(names, [("02335 Operating systems", 2), ("02323 Statistics", 0)]);

        let now = SystemTime::now();
        assert!(format_item(&list[0], now).ends_with("2 notes  just now"), "{}", format_item(&list[0], now));
        assert!(format_item(&list[1], now).trim_end().ends_with("no notes"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn obsidian_url_encodes_the_path() {
        assert_eq!(
            obsidian_url(Path::new("/Users/me/DTU/02335 OS/02335 OS Notes")),
            "obsidian://open?path=%2FUsers%2Fme%2FDTU%2F02335%20OS%2F02335%20OS%20Notes"
        );
    }
}
