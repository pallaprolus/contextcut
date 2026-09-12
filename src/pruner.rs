use std::fs;
use std::path::Path;

use anyhow::Result;
use content_inspector::{ContentType, inspect};
use globset::{Glob, GlobSet, GlobSetBuilder};

/// Lockfiles carry no architectural signal and burn thousands of tokens.
const LOCKFILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "uv.lock",
    "Pipfile.lock",
    "Gemfile.lock",
    "composer.lock",
    "go.sum",
    "flake.lock",
];

#[derive(Debug)]
pub enum FileDecision {
    Keep(String),
    Truncated(String),
    Skip(SkipReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Binary,
    Lockfile,
    Minified,
    Vendor,
    Filtered,
    Unreadable,
}

/// Compiled --include/--exclude glob filters.
pub struct Filters {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl Filters {
    pub fn new(include: &[String], exclude: &[String]) -> Result<Self> {
        Ok(Self {
            include: build_globset(include)?,
            exclude: build_globset(exclude)?,
        })
    }

    fn passes(&self, rel: &Path) -> bool {
        if let Some(inc) = &self.include
            && !inc.is_match(rel)
        {
            return false;
        }
        if let Some(exc) = &self.exclude
            && exc.is_match(rel)
        {
            return false;
        }
        true
    }
}

fn build_globset(patterns: &[String]) -> Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(Some(builder.build()?))
}

/// Decide whether a file is packed, truncated, or skipped.
pub fn decide(path: &Path, rel: &Path, filters: &Filters, max_bytes: u64) -> FileDecision {
    if let Some(reason) = skip_path(rel, filters) {
        return FileDecision::Skip(reason);
    }
    let Ok(bytes) = fs::read(path) else {
        return FileDecision::Skip(SkipReason::Unreadable);
    };
    decide_bytes(&bytes, rel, filters, max_bytes)
}

/// Apply the same pruning to historical content of deleted files.
pub fn decide_bytes(bytes: &[u8], rel: &Path, filters: &Filters, max_bytes: u64) -> FileDecision {
    if let Some(reason) = skip_path(rel, filters) {
        return FileDecision::Skip(reason);
    }
    if inspect(&bytes[..bytes.len().min(1024)]) == ContentType::BINARY {
        return FileDecision::Skip(SkipReason::Binary);
    }

    let content = String::from_utf8_lossy(bytes);
    if (content.len() as u64) > max_bytes {
        return FileDecision::Truncated(truncate(&content, max_bytes as usize));
    }
    FileDecision::Keep(content.into_owned())
}

fn skip_path(rel: &Path, filters: &Filters) -> Option<SkipReason> {
    let name = rel
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();

    if LOCKFILES.iter().any(|l| *l == name) {
        return Some(SkipReason::Lockfile);
    }
    if is_minified(&name) {
        return Some(SkipReason::Minified);
    }
    if !filters.passes(rel) {
        return Some(SkipReason::Filtered);
    }

    None
}

fn is_minified(name: &str) -> bool {
    name.ends_with(".min.js") || name.ends_with(".min.css") || name.ends_with(".map")
}

/// Cut `content` at the largest char boundary <= `cap` and append a marker.
fn truncate(content: &str, cap: usize) -> String {
    let mut cut = cap.min(content.len());
    while !content.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(
        "{}\n... [truncated: {} of {} bytes]",
        &content[..cut],
        cut,
        content.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn no_filters() -> Filters {
        Filters::new(&[], &[]).unwrap()
    }

    fn decide_named(dir: &Path, name: &str, content: &[u8]) -> FileDecision {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        decide(&path, &PathBuf::from(name), &no_filters(), 65536)
    }

    #[test]
    fn skips_lockfiles_exactly() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            decide_named(dir.path(), "poetry.lock", b"a"),
            FileDecision::Skip(SkipReason::Lockfile)
        ));
        assert!(matches!(
            decide_named(dir.path(), "Cargo.lock", b"a"),
            FileDecision::Skip(SkipReason::Lockfile)
        ));
        // No substring false positives.
        assert!(matches!(
            decide_named(dir.path(), "flock.py", b"a"),
            FileDecision::Keep(_)
        ));
    }

    #[test]
    fn skips_minified_assets() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            decide_named(dir.path(), "app.min.js", b"x"),
            FileDecision::Skip(SkipReason::Minified)
        ));
        assert!(matches!(
            decide_named(dir.path(), "bundle.js.map", b"x"),
            FileDecision::Skip(SkipReason::Minified)
        ));
        assert!(matches!(
            decide_named(dir.path(), "admin.js", b"x"),
            FileDecision::Keep(_)
        ));
    }

    #[test]
    fn detects_binary_by_nul_byte() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            decide_named(dir.path(), "blob.bin", b"\x00\x01\x02data"),
            FileDecision::Skip(SkipReason::Binary)
        ));
        // UTF-8 with BOM is still text.
        assert!(matches!(
            decide_named(dir.path(), "bom.txt", "\u{feff}hello".as_bytes()),
            FileDecision::Keep(_)
        ));
    }

    #[test]
    fn truncates_without_splitting_chars() {
        // 'é' is 2 bytes; cap lands mid-char and must back off, not panic.
        let content = "é".repeat(100);
        let result = truncate(&content, 33);
        assert!(result.contains("[truncated: 32 of 200 bytes]"));
        assert!(result.starts_with(&"é".repeat(16)));
    }

    #[test]
    fn truncates_oversize_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.py");
        fs::write(&path, "x".repeat(200)).unwrap();
        let decision = decide(&path, &PathBuf::from("big.py"), &no_filters(), 100);
        match decision {
            FileDecision::Truncated(content) => {
                assert!(content.contains("[truncated: 100 of 200 bytes]"));
            }
            other => panic!("expected Truncated, got {other:?}"),
        }
    }

    #[test]
    fn include_and_exclude_globs() {
        let dir = tempfile::tempdir().unwrap();
        let py_only = Filters::new(&["**/*.py".into()], &[]).unwrap();
        let path = dir.path().join("a.js");
        fs::write(&path, "x").unwrap();
        assert!(matches!(
            decide(&path, &PathBuf::from("a.js"), &py_only, 65536),
            FileDecision::Skip(SkipReason::Filtered)
        ));

        let no_md = Filters::new(&[], &["**/*.md".into()]).unwrap();
        let md = dir.path().join("README.md");
        fs::write(&md, "x").unwrap();
        assert!(matches!(
            decide(&md, &PathBuf::from("README.md"), &no_md, 65536),
            FileDecision::Skip(SkipReason::Filtered)
        ));
    }
}
