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

/// Walk `root` and return a sorted list of candidate file paths.
///
/// Respects .gitignore/.ignore unless `no_gitignore` is set; vendor and
/// cache directories are always skipped. Sorting keeps output (and insta
/// snapshots) deterministic across filesystems.
pub fn walk(root: &Path, no_gitignore: bool) -> Result<Vec<PathBuf>> {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false) // we want e.g. .github/workflows; .git is pruned below
        .git_ignore(!no_gitignore)
        .git_global(!no_gitignore)
        .git_exclude(!no_gitignore)
        .ignore(!no_gitignore)
        .parents(!no_gitignore)
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            let name = entry.file_name().to_string_lossy();
            !(is_dir && (VENDOR_DIRS.contains(&name.as_ref()) || name.ends_with(".egg-info")))
        });

    let mut paths: Vec<PathBuf> = builder
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
        .map(|entry| entry.into_path())
        .collect();
    paths.sort();
    Ok(paths)
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

        let got = names(&walk(root, false).unwrap(), root);
        assert!(got.contains(&"keep.py".to_string()));
        assert!(!got.iter().any(|p| p.contains("secrets")));
        assert!(!got.contains(&"debug.log".to_string()));
    }

    #[test]
    fn no_gitignore_flag_disables_ignore_rules() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        touch(&root.join("debug.log"));

        let got = names(&walk(root, true).unwrap(), root);
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

        let got = names(&walk(root, false).unwrap(), root);
        assert_eq!(got, vec!["app.py".to_string()]);
    }

    #[test]
    fn output_is_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("z.py"));
        touch(&root.join("a.py"));
        touch(&root.join("m/b.py"));

        let got = walk(root, false).unwrap();
        let mut sorted = got.clone();
        sorted.sort();
        assert_eq!(got, sorted);
    }
}
