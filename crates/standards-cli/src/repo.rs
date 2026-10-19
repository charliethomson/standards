//! Finding the repo a command runs in.

use std::path::{Path, PathBuf};

/// The nearest directory at or above `start` holding a `.git` (a directory, or the file a
/// worktree or submodule has).
#[must_use]
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_nearest_git_dir_or_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let deep = root.join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(find_root(&deep), None);
        std::fs::create_dir(root.join(".git")).unwrap();
        assert_eq!(find_root(&deep), Some(root.clone()));
        let wt = deep.join("wt");
        std::fs::create_dir(&wt).unwrap();
        std::fs::write(wt.join(".git"), "gitdir: elsewhere").unwrap();
        assert_eq!(find_root(&wt), Some(wt));
    }
}
