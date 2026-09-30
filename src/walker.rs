use std::path::{Path, PathBuf};

use anyhow::Result;
use ignore::WalkBuilder;

/// Directories that are pruned during the walk even when not gitignored.
/// These never contain content worth sending to an LLM.
const VENDOR_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    "target",
    ".pytest_cache",
    ".ruff_cache",
    ".mypy_cache",
    ".idea",
    ".vscode",
];

/// Walk `root` and return sorted candidate paths and an unreadable count.
///
/// Respects .gitignore/.ignore unless `no_gitignore` is set; vendor and
/// cache directories are always skipped. Sorting keeps output (and insta
/// snapshots) deterministic across filesystems.
pub struct Walk {
    pub paths: Vec<PathBuf>,
    pub unreadable: usize,
}

pub fn walk(root: &Path, no_gitignore: bool) -> Result<Walk> {
    // Outside any git repo, still honor the root's own .gitignore, but never
    // ancestors' (e.g. a dotfiles-style ~/.gitignore). Inside a repo keep
    // git semantics so repo boundaries and .git/info/exclude hold.
    let in_git = inside_git_repo(root);
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false) // we want e.g. .github/workflows; .git is pruned below
        .require_git(in_git)
        .git_ignore(!no_gitignore)
        .git_global(!no_gitignore)
        .git_exclude(!no_gitignore)
        .ignore(!no_gitignore)
        .parents(!no_gitignore && in_git)
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            let name = entry.file_name().to_string_lossy();
            !(is_dir && (VENDOR_DIRS.contains(&name.as_ref()) || name.ends_with(".egg-info")))
        });

    let mut paths = Vec::new();
    let mut unreadable = 0;
    for entry in builder.build() {
        match entry {
            Ok(entry) if entry.file_type().is_some_and(|t| t.is_file()) => {
                paths.push(entry.into_path());
            }
            Err(_) => unreadable += 1,
            _ => {}
        }
    }
    paths.sort();
    Ok(Walk { paths, unreadable })
}

/// Whether `root` or any ancestor has a `.git` entry (dir or worktree file).
fn inside_git_repo(root: &Path) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    root.ancestors().any(|dir| dir.join(".git").exists())
}

/// Match a deleted path against current ignore rules without restoring it.
pub fn allows_deleted(root: &Path, rel: &Path, no_gitignore: bool) -> Result<bool> {
    if rel.components().any(|part| {
        let name = part.as_os_str().to_string_lossy();
        VENDOR_DIRS.contains(&name.as_ref()) || name.ends_with(".egg-info")
    }) {
        return Ok(false);
    }
    if no_gitignore {
        return Ok(true);
    }
    if crate::diff::is_ignored(root, rel)? {
        return Ok(false);
    }
    // Git handles .gitignore/global/exclude; the walker also supports .ignore.
    let full = root.canonicalize()?.join(rel);
    let mut ignored = false;
    let mut ancestors: Vec<_> = full
        .parent()
        .into_iter()
        .flat_map(|p| p.ancestors())
        .collect();
    ancestors.reverse();
    for dir in ancestors {
        let file = dir.join(".ignore");
        if file.is_file() {
            let mut builder = ignore::gitignore::GitignoreBuilder::new(dir);
            if let Some(err) = builder.add(file) {
                return Err(err.into());
            }
            let rules = builder.build()?;
            match rules.matched_path_or_any_parents(&full, false) {
                ignore::Match::Ignore(_) => ignored = true,
                ignore::Match::Whitelist(_) => ignored = false,
                ignore::Match::None => {}
            }
        }
    }
    Ok(!ignored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
    }

    fn names(paths: &[PathBuf], root: &Path) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // `ignore` only honors .gitignore inside a git repo context.
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "secrets/\n*.log\n").unwrap();
        touch(&root.join("keep.py"));
        touch(&root.join("secrets/key.txt"));
        touch(&root.join("debug.log"));

        let got = names(&walk(root, false).unwrap().paths, root);
        assert!(got.contains(&"keep.py".to_string()));
        assert!(!got.iter().any(|p| p.contains("secrets")));
        assert!(!got.contains(&"debug.log".to_string()));
    }

    #[test]
    fn respects_gitignore_outside_git_repo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        touch(&root.join("keep.py"));
        touch(&root.join("debug.log"));

        let got = names(&walk(root, false).unwrap().paths, root);
        assert!(got.contains(&"keep.py".to_string()));
        assert!(!got.contains(&"debug.log".to_string()));
        assert!(names(&walk(root, true).unwrap().paths, root).contains(&"debug.log".to_string()));
    }

    #[test]
    fn no_gitignore_flag_disables_ignore_rules() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        touch(&root.join("debug.log"));

        let got = names(&walk(root, true).unwrap().paths, root);
        assert!(got.contains(&"debug.log".to_string()));
    }

    #[test]
    fn always_prunes_vendor_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("app.py"));
        touch(&root.join(".venv/lib/site.py"));
        touch(&root.join("node_modules/x/index.js"));
        touch(&root.join("pkg.egg-info/PKG-INFO"));

        let got = names(&walk(root, false).unwrap().paths, root);
        assert_eq!(got, vec!["app.py".to_string()]);
    }

    #[test]
    fn output_is_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("z.py"));
        touch(&root.join("a.py"));
        touch(&root.join("m/b.py"));

        let got = walk(root, false).unwrap().paths;
        let mut sorted = got.clone();
        sorted.sort();
        assert_eq!(got, sorted);
    }

    #[test]
    fn non_git_root_ignores_ancestor_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.py\n").unwrap();
        let root = dir.path().join("proj");
        touch(&root.join("a.py"));
        let got = names(&walk(&root, false).unwrap().paths, &root);
        assert_eq!(got, vec!["a.py".to_string()]);
    }

    #[test]
    fn repo_ignores_gitignore_above_repo_root() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "*\n").unwrap();
        let root = dir.path().join("proj");
        fs::create_dir_all(root.join(".git")).unwrap();
        touch(&root.join("a.py"));
        let got = names(&walk(&root, false).unwrap().paths, &root);
        assert_eq!(got, vec!["a.py".to_string()]);
    }

    #[test]
    fn counts_walk_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let got = walk(&missing, false).unwrap();
        assert!(got.paths.is_empty());
        assert_eq!(got.unreadable, 1);
    }
}
