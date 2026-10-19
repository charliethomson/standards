//! File and process helpers the checks share: atomic writes, symlink targets, `PATH`
//! lookup, and running `git` / `claude`.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

/// Replace `path` with `bytes`: a temp file in the same directory, then a rename. An
/// existing file's permissions carry over.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    let tmp = dir.join(format!(".{name}.standards.{}", std::process::id()));
    let result = (|| {
        std::fs::write(&tmp, bytes).map_err(|e| Error::io(&tmp, e))?;
        if let Ok(meta) = std::fs::metadata(path) {
            std::fs::set_permissions(&tmp, meta.permissions()).map_err(|e| Error::io(&tmp, e))?;
        }
        std::fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The target of `path` when it is a symlink.
#[must_use]
pub fn link_target(path: &Path) -> Option<PathBuf> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() {
        std::fs::read_link(path).ok()
    } else {
        None
    }
}

/// Whether `target` passes through the directories `run` in order (`["standards",
/// "skills"]` matches `../../standards/skills/x` and `/home/u/git/standards/skills/x`).
#[must_use]
pub fn points_into(target: &Path, run: &[&str]) -> bool {
    let names: Vec<&OsStr> = target
        .components()
        .filter_map(|c| match c {
            Component::Normal(n) => Some(n),
            _ => None,
        })
        .collect();
    names
        .windows(run.len())
        .any(|w| w.iter().zip(run).all(|(a, b)| *a == OsStr::new(b)))
}

/// The first executable `name` on `path`.
#[must_use]
pub fn which(name: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path?)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Run `cmd`; its stdout on success, else its stderr (or stdout) as the error.
pub fn output(cmd: &mut Command, what: &str) -> Result<String> {
    let out = cmd
        .output()
        .map_err(|e| Error::Fail(format!("{what}: {e}")))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let said = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    Err(Error::Fail(format!("{what}: {}", said.trim())))
}

/// `git -C root <args>`.
pub fn git(root: &Path, args: &[&str]) -> Result<String> {
    output(
        Command::new("git").arg("-C").arg(root).args(args),
        &format!("git {}", args.join(" ")),
    )
}

/// The repo's shared git dir (`.git`, or the main checkout's for a worktree).
pub fn git_common_dir(root: &Path) -> Result<PathBuf> {
    let dir = git(root, &["rev-parse", "--git-common-dir"])?;
    let dir = PathBuf::from(dir.trim());
    Ok(if dir.is_absolute() {
        dir
    } else {
        root.join(dir)
    })
}

/// The paths under `pathspec` that git tracks, relative to `root`.
pub fn tracked(root: &Path, pathspec: &str) -> Result<Vec<String>> {
    let out = git(root, &["ls-files", "-z", "--", pathspec])?;
    Ok(out
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Remove `rel` (under `root`) from the index and the tree when tracked, else just from
/// the tree. Says which.
pub fn remove(root: &Path, rel: &str) -> Result<String> {
    if tracked(root, rel)?.iter().any(|p| p == rel) {
        git(root, &["rm", "-q", "-f", "--", rel])?;
        Ok(format!("git rm {rel}"))
    } else {
        let path = root.join(rel);
        std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))?;
        Ok(format!("removed {rel}"))
    }
}

/// `~/x` for a path under `home`, else the path.
#[must_use]
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_into_matches_component_runs() {
        let run = ["standards", "skills"];
        assert!(points_into(Path::new("../../standards/skills/x"), &run));
        assert!(points_into(Path::new("/h/git/standards/skills/x"), &run));
        assert!(!points_into(Path::new("/h/git/standards-x/skills/x"), &run));
        assert!(!points_into(Path::new("/h/skills/standards"), &run));
        assert!(points_into(
            Path::new("standards/bin/ultron"),
            &["standards", "bin"]
        ));
    }

    #[test]
    fn which_needs_an_executable_on_an_absolute_dir() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("tool");
        std::fs::write(&file, "#!/bin/sh\n").unwrap();
        let path = std::env::join_paths([PathBuf::from("rel"), dir.path().to_owned()]).unwrap();
        #[cfg(unix)]
        {
            assert_eq!(which("tool", Some(&path)), None);
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(which("tool", Some(&path)), Some(file));
        assert_eq!(which("tool", None), None);
        assert_eq!(which("nope", Some(&path)), None);
    }

    #[test]
    fn atomic_write_keeps_permissions_and_leaves_no_temp() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/f.json");
        write_atomic(&path, b"one").unwrap();
        #[cfg(unix)]
        {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            write_atomic(&path, b"two").unwrap();
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert_eq!(std::fs::read(&path).unwrap(), b"two".to_vec());
        assert_eq!(std::fs::read_dir(dir.path().join("a")).unwrap().count(), 1);
        // A directory in the way: the rename fails and the temp is cleaned up.
        let blocked = dir.path().join("b");
        std::fs::create_dir_all(blocked.join("x")).unwrap();
        assert!(write_atomic(&blocked, b"x").is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn tilde_and_failing_commands() {
        assert_eq!(
            tilde(Path::new("/h/.claude/x"), Path::new("/h")),
            "~/.claude/x"
        );
        assert_eq!(tilde(Path::new("/o/x"), Path::new("/h")), "/o/x");
        let e = output(&mut Command::new("/nonexistent/bin"), "run it").unwrap_err();
        assert!(e.to_string().starts_with("run it: "), "{e}");
        let dir = tempfile::tempdir().unwrap();
        let e = git(dir.path(), &["rev-parse", "--git-common-dir"]).unwrap_err();
        assert!(e.to_string().starts_with("git rev-parse"), "{e}");
    }
}
