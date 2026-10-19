//! The state directory on disk: which programs exist, how recently each was touched, and
//! the fingerprint `watch` redraws on. Read-only except for [`StateDir::remove`] (`gc`).
//!
//! Paths are built by string concatenation, as the script builds them, so the paths it
//! prints are byte-identical (`$STATE_DIR/$slug/$id.md`).

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::{Error, Result};

pub(crate) struct StateDir {
    root: String,
}

/// `die` in the script: an `error: …` line and exit 1.
pub(crate) fn die(msg: impl Into<String>) -> Error {
    Error::Fail(msg.into())
}

impl StateDir {
    pub(crate) fn new(root: String) -> Self {
        Self { root }
    }

    pub(crate) fn root(&self) -> &str {
        &self.root
    }

    /// `$STATE_DIR/<slug>`, whether or not it exists.
    pub(crate) fn path_of(&self, slug: &str) -> String {
        format!("{}/{slug}", self.root)
    }

    /// The program's directory, or the script's "no such program" error.
    pub(crate) fn program_dir(&self, slug: &str) -> Result<String> {
        let dir = self.path_of(slug);
        if Path::new(&dir).is_dir() {
            Ok(dir)
        } else {
            Err(die(format!("no such program: {slug} (try 'ultron ls')")))
        }
    }

    /// Programs, most recently touched first (`sort -rn`: ties fall back to the whole line,
    /// reversed, so the later name first).
    pub(crate) fn list_programs(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut progs: Vec<(i64, String)> = entries
            .filter_map(std::result::Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                // `"$STATE_DIR"/*/`: no dotfiles, directories (or links to them) only.
                (!name.starts_with('.') && e.path().is_dir()).then_some(name)
            })
            .map(|name| (newest(&self.path_of(&name)), name))
            .collect();
        progs.sort_by(|a, b| b.cmp(a));
        progs.into_iter().map(|(_, name)| name).collect()
    }

    /// The most recently touched program, for `status` / `watch` with no argument.
    pub(crate) fn default_program(&self) -> Result<String> {
        self.list_programs()
            .into_iter()
            .next()
            .ok_or_else(|| die(format!("no programs under {}", self.root)))
    }

    /// `find "$STATE_DIR" -name "$id.md" -type f`, in a stable (sorted) order.
    pub(crate) fn journals_named(&self, id: &str) -> Vec<String> {
        let want = format!("{id}.md");
        files_under(&self.root)
            .into_iter()
            .filter(|p| p.rsplit('/').next() == Some(want.as_str()))
            .collect()
    }

    pub(crate) fn remove(dir: &str) -> Result<()> {
        fs::remove_dir_all(dir).map_err(|e| Error::io(Path::new(dir), e))
    }
}

/// mtime in whole seconds, without following a symlink (GNU `stat -c %Y`); 0 if absent.
pub(crate) fn mtime(path: &str) -> i64 {
    fs::symlink_metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .unwrap_or(0)
}

/// `find <dir> -type f`: regular files at any depth, sorted, never through a symlink (not
/// even when `dir` itself is one, as with find's default `-P`).
pub(crate) fn files_under(dir: &str) -> Vec<String> {
    fn walk(dir: &str, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut names: Vec<_> = entries
            .filter_map(std::result::Result::ok)
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.file_type()))
            .collect();
        names.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, ty) in names {
            let path = format!("{dir}/{name}");
            match ty {
                Ok(t) if t.is_dir() => walk(&path, out),
                Ok(t) if t.is_file() => out.push(path),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    if fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir()) {
        walk(dir, &mut out);
    }
    out.sort();
    out
}

/// The newest mtime of any file under `dir`; 0 when there is none.
pub(crate) fn newest(dir: &str) -> i64 {
    files_under(dir)
        .iter()
        .map(|f| mtime(f))
        .max()
        .unwrap_or(0)
        .max(0)
}

/// A cheap signature of a program's state: every file and its mtime. `watch` redraws only
/// when it changes, so a quiet program doesn't flicker.
pub(crate) fn fingerprint(dir: &str) -> String {
    files_under(dir).iter().fold(String::new(), |mut out, f| {
        let _ = write!(out, "{f}:{};", mtime(f));
        out
    })
}

/// `epoch` -> `just now` / `3m ago` / `2h ago` / `4d ago`; `never` for 0.
pub(crate) fn ago(then: i64, now: i64) -> String {
    if then == 0 {
        return "never".into();
    }
    let delta = now - then;
    if delta < 60 {
        "just now".into()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86400)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn ago_buckets() {
        assert_eq!(ago(0, 100), "never");
        assert_eq!(ago(100, 90), "just now");
        assert_eq!(ago(100, 159), "just now");
        assert_eq!(ago(100, 160), "1m ago");
        assert_eq!(ago(1, 3600), "59m ago");
        assert_eq!(ago(1, 3601), "1h ago");
        assert_eq!(ago(1, 86400), "23h ago");
        assert_eq!(ago(1, 86401 + 86400), "2d ago");
    }

    #[test]
    fn missing_dirs_are_empty_not_errors() {
        let s = StateDir::new("/nonexistent/thmsn/ultron".into());
        assert_eq!(s.list_programs(), Vec::<String>::new());
        assert_eq!(newest("/nonexistent"), 0);
        assert_eq!(fingerprint("/nonexistent"), "");
        assert_eq!(mtime("/nonexistent"), 0);
        let e = s.default_program().unwrap_err().to_string();
        assert_eq!(e, "no programs under /nonexistent/thmsn/ultron");
        let e = s.program_dir("x").unwrap_err().to_string();
        assert_eq!(e, "no such program: x (try 'ultron ls')");
        assert!(StateDir::remove("/nonexistent/x").is_err());
    }

    #[test]
    fn fingerprint_tracks_files_and_mtimes_only() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_str().unwrap().to_owned();
        let a = format!("{dir}/a.md");
        fs::write(&a, "x").unwrap();
        fs::create_dir(format!("{dir}/sub")).unwrap();
        fs::write(format!("{dir}/sub/b"), "y").unwrap();
        let f0 = fingerprint(&dir);
        assert!(f0.starts_with(&format!("{dir}/a.md:")));
        assert!(f0.contains(&format!("{dir}/sub/b:")));
        // Rewriting the same mtime is no change; a new mtime or a new file is.
        fs::write(&a, "changed").unwrap();
        set_mtime(&a, 1_000);
        let f1 = fingerprint(&dir);
        assert_ne!(f0, f1);
        fs::write(&a, "again").unwrap();
        set_mtime(&a, 1_000);
        assert_eq!(fingerprint(&dir), f1);
        fs::write(format!("{dir}/c"), "").unwrap();
        assert_ne!(fingerprint(&dir), f1);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap().to_owned();
        fs::create_dir(format!("{root}/real")).unwrap();
        fs::write(format!("{root}/real/f"), "").unwrap();
        std::os::unix::fs::symlink(format!("{root}/real"), format!("{root}/link")).unwrap();
        std::os::unix::fs::symlink(format!("{root}/real/f"), format!("{root}/real/g")).unwrap();
        assert_eq!(files_under(&format!("{root}/link")), Vec::<String>::new());
        assert_eq!(files_under(&root), vec![format!("{root}/real/f")]);
        // ...but `*/` lists a linked program, as the glob does.
        let s = StateDir::new(root.clone());
        assert_eq!(
            s.list_programs(),
            vec!["real".to_owned(), "link".to_owned()]
        );
    }

    pub(crate) fn set_mtime(path: &str, epoch: u64) {
        let t = UNIX_EPOCH + std::time::Duration::from_secs(epoch);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }
}
