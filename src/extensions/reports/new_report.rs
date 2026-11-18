use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::template::{self, Fields};
use super::{Settings, courses, settings};
use crate::actions::{Action, Ctx, Pick, Prompt, Step, open_project};
use crate::extensions::folders::{self, course_number};

pub struct NewReport;

impl Action for NewReport {
    fn key(&self) -> char {
        'n'
    }

    fn title(&self) -> &str {
        "New report"
    }

    fn run(&self, ctx: &Ctx) -> Result<Step> {
        let settings = settings(ctx.config)?;
        if settings.name.trim().is_empty() {
            bail!("set your name first, under Config → Extensions → Reports");
        }
        course_picker(ctx, 0)
    }
}

/// Step 1: choose the course.
fn course_picker(ctx: &Ctx, selected: usize) -> Result<Step> {
    let settings = settings(ctx.config)?;
    let courses = courses(&settings)?;
    if courses.is_empty() {
        return Ok(Step::Done(format!("no course folders in {}", settings.root.display())));
    }
    let items = courses.iter().map(|c| folders::truncate(&c.name, 46)).collect();
    let courses: Vec<(String, PathBuf)> = courses.into_iter().map(|c| (c.name, c.path)).collect();

    Ok(Pick::new("Course", items, move |i, _ctx| {
        let (course, course_dir) = courses[i].clone();
        Ok(name_prompt(course, course_dir, i))
    })
    .selected(selected)
    .into())
}

/// Step 2: name the report, then create and open it.
fn name_prompt(course: String, course_dir: PathBuf, course_row: usize) -> Step {
    let label = format!("Report name ({})", course_number(&course));
    Prompt::new(label, move |title, ctx| {
        let settings = settings(ctx.config)?;
        let path = create(&settings, &course, &course_dir, &title)?;
        let folder = path.file_name().unwrap_or_default().to_string_lossy();
        let message = format!("created {folder} in {}", course_number(&course));
        open_project(ctx, &path, &message)
    })
    .back(move |ctx| course_picker(ctx, course_row))
    .into()
}

/// Make `<course>/<assignments_dir>/<Title-With-Dashes>/` from the template.
fn create(settings: &Settings, course: &str, course_dir: &Path, title: &str) -> Result<PathBuf> {
    let title = folders::validate_name(title)?;
    let folder = title.split_whitespace().collect::<Vec<_>>().join("-");
    let path = course_dir.join(&settings.assignments_dir).join(folder);
    folders::create_new(&path)?;

    let fields = Fields { title, subtitle: course, author: settings.name.trim() };
    if let Err(err) = template::write(&path, &fields) {
        // Don't leave a half-made report behind; the user can retry the same name.
        let _ = fs::remove_dir_all(&path);
        return Err(err);
    }
    Ok(path)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::folders::tests::temp_dir;

    #[test]
    fn creates_report_from_template() {
        let root = temp_dir("reports-new");
        let course = "02323 Introduction to Statistics";
        let course_dir = root.join(course);
        fs::create_dir(&course_dir).unwrap();
        let settings = Settings {
            name: "Ada Lovelace".into(),
            root: root.clone(),
            assignments_dir: "Assignments".into(),
            recent_count: 10,
        };

        let path = create(&settings, course, &course_dir, "  Lab  report 2 ").unwrap();
        assert_eq!(path, course_dir.join("Assignments/Lab-report-2"));
        assert!(path.join("main.typ").is_file());
        assert!(path.join("src/images/image.webp").is_file());
        let filled = fs::read_to_string(path.join("src/report-settings.typ")).unwrap();
        assert!(filled.contains("#let title = \"Lab  report 2\""), "{filled}");
        assert!(filled.contains("#let subtitle = \"02323 Introduction to Statistics\""));
        assert!(filled.contains("#let author = \"Ada Lovelace\""));

        let err = create(&settings, course, &course_dir, "Lab report 2").unwrap_err();
        assert_eq!(err.to_string(), "Lab-report-2 already exists");
        assert!(create(&settings, course, &course_dir, "a/b").is_err());

        fs::remove_dir_all(&root).unwrap();
    }
}
