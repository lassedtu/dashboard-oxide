//! Folder helpers shared by extensions that create and list folders
//! (coding projects, reports).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

pub struct Folder {
    pub name: String,
    pub path: PathBuf,
    pub last_activity: SystemTime,
}

/// Non-hidden folders directly inside `dir`, in no particular order.
pub fn subfolders(dir: &Path) -> Result<Vec<Folder>> {
    let entries = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    Ok(entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if name.starts_with('.') {
                return None;
            }
            let path = e.path();
            let last_activity = last_activity(&path)?;
            Some(Folder { name, path, last_activity })
        })
        .collect())
}

/// Sort most recently active first and keep the first `limit`.
pub fn newest_first(folders: &mut Vec<Folder>, limit: usize) {
    folders.sort_by_key(|f| std::cmp::Reverse(f.last_activity));
    folders.truncate(limit);
}

/// A folder's own mtime only changes when direct children are added/removed,
/// so also look at git's index and HEAD, which change on staging/commits/checkouts.
fn last_activity(path: &Path) -> Option<SystemTime> {
    [path.to_path_buf(), path.join(".git/index"), path.join(".git/HEAD")]
        .iter()
        .filter_map(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
        .max()
}

/// Course folders in `root` (for Reports and Notes), by name. A course
/// folder starts with its course number ("02323 Introduction to
/// Statistics"), so "Misc", "Old Courses" and the like are left out.
pub fn courses(root: &Path) -> Result<Vec<Folder>> {
    let mut courses: Vec<Folder> = subfolders(root)?.into_iter().filter(|f| is_course(&f.name)).collect();
    courses.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(courses)
}

fn is_course(name: &str) -> bool {
    let number = course_number(name);
    !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
}

/// "02323" from "02323 Introduction to Statistics".
pub fn course_number(course: &str) -> &str {
    course.split_whitespace().next().unwrap_or(course)
}

/// A name typed by the user, checked to be usable as a single folder name.
pub fn validate_name(raw: &str) -> Result<&str> {
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

/// Create `path` as a new, empty folder (its parents may already exist).
/// Fails rather than reuse an existing folder, so nothing gets overwritten.
pub fn create_new(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::create_dir(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            anyhow::anyhow!("{name} already exists")
        }
        _ => anyhow::Error::new(e).context(format!("creating {}", path.display())),
    })
}

/// Cut to `width` characters, ending with "…" if shortened.
pub fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    text.chars().take(width - 1).collect::<String>() + "…"
}

pub fn humanize_age(t: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(t).unwrap_or_default().as_secs();
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::time::Duration;

    /// A fresh, empty temp folder, unique per test name.
    pub fn temp_dir(name: &str) -> PathBuf {
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

        let mut folders = subfolders(&dir).unwrap();
        newest_first(&mut folders, 10);
        let names: Vec<_> = folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["active", "old"]);

        newest_first(&mut folders, 1);
        assert_eq!(folders.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn create_new_refuses_existing() {
        let dir = temp_dir("create-new");
        create_new(&dir.join("a/b")).unwrap();
        let err = create_new(&dir.join("a/b")).unwrap_err();
        assert_eq!(err.to_string(), "b already exists");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn validates_names() {
        assert_eq!(validate_name("  my-app ").unwrap(), "my-app");
        assert!(validate_name("   ").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("..").is_err());
        assert!(validate_name(".hidden").is_err());
    }

    #[test]
    fn course_folders_start_with_a_number() {
        assert!(is_course("02323 Introduction to Statistics"));
        assert!(is_course("62550 UX"));
        assert!(!is_course("Old Courses"));
        assert!(!is_course("Misc"));
        assert!(!is_course("2023-notes"));
        assert_eq!(course_number("02335 Operating systems"), "02335");
    }

    #[test]
    fn humanizes_ages() {
        let now = SystemTime::now();
        let ago = |s| now - Duration::from_secs(s);
        assert_eq!(humanize_age(ago(5), now), "just now");
        assert_eq!(humanize_age(ago(120), now), "2m ago");
        assert_eq!(humanize_age(ago(7200), now), "2h ago");
        assert_eq!(humanize_age(ago(3 * 86400), now), "3d ago");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
    }
}
