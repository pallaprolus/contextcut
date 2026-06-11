use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Copy the checked-in fixture into a tempdir, activating its gitignore.
///
/// The fixture stores `gitignore.txt` (a real `.gitignore` would be subject
/// to *this* repo's git context and confuse the `ignore` crate); the copy
/// renames it and plants a `.git` dir so ignore rules take effect — hermetic
/// regardless of where the repo is checked out.
fn setup() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini-repo");
    copy_tree(&src, tmp.path());
    fs::rename(tmp.path().join("gitignore.txt"), tmp.path().join(".gitignore")).unwrap();
    fs::create_dir(tmp.path().join(".git")).unwrap();
    tmp
}

fn copy_tree(src: &Path, dst: &Path) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir_all(&to).unwrap();
            copy_tree(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

fn contextcut() -> Command {
    Command::cargo_bin("contextcut").unwrap()
}

#[test]
fn default_pack_includes_code_and_excludes_noise() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("```python")
                .and(predicate::str::contains("def main"))
                .and(predicate::str::contains("poetry.lock").not())
                .and(predicate::str::contains("SUPER-SECRET-KEY").not())
                .and(predicate::str::contains("secrets/key.txt").not())
                .and(predicate::str::contains("minified-noise").not())
                .and(predicate::str::contains("logo.png").not())
                .and(predicate::str::contains("log-noise").not()),
        )
        .stderr(predicate::str::contains("Estimated tokens"));
}

#[test]
fn fence_collision_uses_longer_fence() {
    let repo = setup();
    // util.js contains a ``` inside a string; its block must use ````.
    contextcut()
        .arg(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("````javascript"));
}

#[test]
fn output_flag_writes_file_and_keeps_stdout_empty() {
    let repo = setup();
    let out = repo.path().join("packed.md");
    contextcut()
        .arg(repo.path())
        .args(["-o", out.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Estimated tokens"));
    let written = fs::read_to_string(&out).unwrap();
    assert!(written.contains("```python"));
}

#[test]
fn tokens_only_emits_no_markdown() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .arg("--tokens-only")
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Files packed").and(predicate::str::contains("o200k")));
}

#[test]
fn include_glob_filters_to_matching_files() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .args(["--include", "**/*.py"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("def main").and(predicate::str::contains("javascript").not()),
        );
}

#[test]
fn exclude_glob_drops_matching_files() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .args(["--exclude", "**/*.md"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mini-repo\n\n## File tree").not().or(
            predicate::str::contains("Test fixture for contextcut").not(),
        ));
}

#[test]
fn oversize_files_get_truncation_marker() {
    let repo = setup();
    fs::write(repo.path().join("big.py"), "x = 1\n".repeat(400)).unwrap();
    contextcut()
        .arg(repo.path())
        .args(["--max-file-size", "1kb"])
        .assert()
        .success()
        .stdout(predicate::str::contains("... [truncated: 1024 of 2400 bytes]"));
}

#[test]
fn strip_comments_removes_comment_lines_keeps_code() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .arg("--strip-comments")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("This comment should vanish")
                .not()
                .and(predicate::str::contains("Full-line comment, removable").not())
                .and(predicate::str::contains("# not a comment, must survive"))
                .and(predicate::str::contains("#!/usr/bin/env python")),
        );
}

#[test]
fn no_gitignore_packs_ignored_files() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .arg("--no-gitignore")
        .assert()
        .success()
        .stdout(predicate::str::contains("log-noise"));
}

#[test]
fn nonexistent_path_fails_with_readable_error() {
    contextcut()
        .arg("/definitely/not/a/real/path")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a directory"));
}
