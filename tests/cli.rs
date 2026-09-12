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
    fs::rename(
        tmp.path().join("gitignore.txt"),
        tmp.path().join(".gitignore"),
    )
    .unwrap();
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
        .stdout(
            predicate::str::contains("mini-repo\n\n## File tree")
                .not()
                .or(predicate::str::contains("Test fixture for contextcut").not()),
        );
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
        .stdout(predicate::str::contains(
            "... [truncated: 1024 of 2400 bytes]",
        ));
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

#[test]
fn related_depth_one_packs_direct_neighbors_only() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .args(["--related", "src/chain_a.py", "--depth", "1"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("chain_a.py")
                .and(predicate::str::contains("chain_b.py"))
                .and(predicate::str::contains("leaf-marker-c").not())
                .and(predicate::str::contains("standalone-island").not())
                .and(predicate::str::contains("def main").not()),
        );
}

#[test]
fn related_depth_two_reaches_transitive_imports() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .args(["--related", "src/chain_a.py"]) // default depth 2
        .assert()
        .success()
        .stdout(
            predicate::str::contains("leaf-marker-c")
                .and(predicate::str::contains("standalone-island").not()),
        );
}

#[test]
fn related_seed_on_leaf_pulls_in_importers() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .args(["--related", "src/chain_c.py", "--depth", "1"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("chain_b.py")
                .and(predicate::str::contains("chain_a.py").not()),
        );
}

#[test]
fn related_unknown_seed_fails_with_readable_error() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .args(["--related", "src/nope.py"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no packed file matches"));
}

#[test]
fn map_section_shows_import_edges() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .arg("--map")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("## Dependency map")
                .and(predicate::str::contains("→ src/chain_b.py"))
                .and(predicate::str::contains("← src/chain_a.py")),
        );
}

/// Turn the fixture copy into a real git repo with everything committed.
fn setup_git() -> TempDir {
    let repo = setup();
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&[
        "-c",
        "user.name=test",
        "-c",
        "user.email=t@t",
        "commit",
        "-qm",
        "fixture baseline",
    ]);
    repo
}

#[test]
fn diff_packs_changed_file_and_blast_radius() {
    let repo = setup_git();
    // Modify the leaf; importers chain_b (1 hop) and chain_a (2 hops) follow.
    fs::write(
        repo.path().join("src/chain_c.py"),
        "def leaf():\n    return \"leaf-marker-c-changed\"\n",
    )
    .unwrap();
    contextcut()
        .arg(repo.path())
        .arg("--diff")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("leaf-marker-c-changed")
                .and(predicate::str::contains("chain_b.py"))
                .and(predicate::str::contains("chain_a.py"))
                .and(predicate::str::contains("standalone-island").not())
                .and(predicate::str::contains("def main").not()),
        );
}

#[test]
fn diff_includes_untracked_files() {
    let repo = setup_git();
    fs::write(repo.path().join("src/newborn.py"), "FRESH_MARKER = 1\n").unwrap();
    contextcut()
        .arg(repo.path())
        .arg("--diff")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("FRESH_MARKER")
                .and(predicate::str::contains("standalone-island").not()),
        );
}

#[test]
fn diff_with_no_changes_fails_with_readable_error() {
    let repo = setup_git();
    contextcut()
        .arg(repo.path())
        .arg("--diff")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no packable changed files"));
}

#[test]
fn diff_outside_git_repo_fails_with_readable_error() {
    let repo = setup(); // fake empty .git dir — not a real repository
    contextcut()
        .arg(repo.path())
        .arg("--diff")
        .assert()
        .failure()
        .stderr(predicate::str::contains("git").and(predicate::str::contains("failed")));
}

#[test]
fn exact_claude_without_key_falls_back_gracefully() {
    let repo = setup();
    contextcut()
        .arg(repo.path())
        .args(["--exact-claude", "--tokens-only"])
        .env_remove("ANTHROPIC_API_KEY")
        .assert()
        .success()
        .stderr(
            predicate::str::contains("falling back to approximation")
                .and(predicate::str::contains("Claude (approx")),
        );
}

fn git(repo: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn review_contains_patch_context_reasons_and_default_budget() {
    let repo = setup_git();
    fs::write(repo.path().join("src/chain_c.py"), "CHANGED_REVIEW = 42\n").unwrap();
    contextcut()
        .arg("review")
        .arg(repo.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("## Patch")
                .and(predicate::str::contains("+CHANGED_REVIEW = 42"))
                .and(predicate::str::contains("## src/chain_b.py"))
                .and(predicate::str::contains("1 hop(s)"))
                .and(predicate::str::contains("standalone-island").not()),
        )
        .stderr(predicate::str::contains("/ 20000 o200k_base tokens"));
}

#[test]
fn review_supports_deleted_files_and_their_importers() {
    let repo = setup_git();
    fs::remove_file(repo.path().join("src/chain_c.py")).unwrap();
    contextcut()
        .arg("review")
        .arg(repo.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("deleted file mode")
                .and(predicate::str::contains("deleted; base revision content"))
                .and(predicate::str::contains("## src/chain_b.py")),
        );
}

#[test]
fn review_untracked_only_and_names_with_spaces() {
    let repo = setup_git();
    fs::write(repo.path().join("a new file.py"), "UNTRACKED_ONLY = True\n").unwrap();
    contextcut()
        .arg("review")
        .arg(repo.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("untracked addition")
                .and(predicate::str::contains("UNTRACKED_ONLY"))
                .and(predicate::str::contains("No tracked text patch")),
        );
}

#[test]
fn review_patch_respects_filters_and_ignored_deleted_paths() {
    let repo = setup_git();
    fs::write(repo.path().join("private.py"), "PRIVATE_CONTENT = 1\n").unwrap();
    git(repo.path(), &["add", "private.py"]);
    git(
        repo.path(),
        &[
            "-c",
            "user.name=test",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "private file",
        ],
    );
    fs::write(repo.path().join(".ignore"), "private.py\n").unwrap();
    fs::remove_file(repo.path().join("private.py")).unwrap();
    fs::write(repo.path().join("src/chain_c.py"), "FILTERED_CONTENT = 1\n").unwrap();
    fs::write(repo.path().join("poetry.lock"), "LOCK_CONTENT\n").unwrap();
    fs::write(repo.path().join("public.py"), "PUBLIC_CONTENT = 1\n").unwrap();
    contextcut()
        .arg("review")
        .arg(repo.path())
        .args(["--exclude", "src/chain_c.py"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("PUBLIC_CONTENT")
                .and(predicate::str::contains("PRIVATE_CONTENT").not())
                .and(predicate::str::contains("FILTERED_CONTENT").not())
                .and(predicate::str::contains("LOCK_CONTENT").not()),
        );
}

#[test]
fn review_subdirectory_scope_and_staged_changes() {
    let repo = setup_git();
    fs::write(repo.path().join("src/chain_c.py"), "STAGED_CHANGE = 1\n").unwrap();
    git(repo.path(), &["add", "src/chain_c.py"]);
    fs::write(repo.path().join("README.md"), "OUTSIDE_SUBDIR_CHANGE\n").unwrap();
    contextcut()
        .arg("review")
        .arg(repo.path().join("src"))
        .assert()
        .success()
        .stdout(
            predicate::str::contains("+STAGED_CHANGE = 1")
                .and(predicate::str::contains("## chain_c.py"))
                .and(predicate::str::contains("OUTSIDE_SUBDIR_CHANGE").not()),
        );
}

#[test]
fn review_explicit_base_and_invalid_reference() {
    let repo = setup_git();
    git(repo.path(), &["branch", "review-base"]);
    fs::write(repo.path().join("src/chain_c.py"), "COMMITTED_CHANGE = 1\n").unwrap();
    git(repo.path(), &["add", "src/chain_c.py"]);
    git(
        repo.path(),
        &[
            "-c",
            "user.name=test",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "change",
        ],
    );
    contextcut()
        .arg("review")
        .arg(repo.path())
        .args(["--base", "review-base"])
        .assert()
        .success()
        .stdout(predicate::str::contains("+COMMITTED_CHANGE = 1"));
    contextcut()
        .arg("review")
        .arg(repo.path())
        .args(["--base", "--help"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("git failed"));
}

#[test]
fn budget_preserves_seed_skips_large_neighbor_and_counts_final_document() {
    let repo = setup();
    fs::write(
        repo.path().join("src/chain_b.py"),
        format!(
            "from . import chain_c\n{}",
            "EXPENSIVE_CONTEXT = 123456789\n".repeat(2000)
        ),
    )
    .unwrap();
    let output = contextcut()
        .arg(repo.path())
        .args(["--related", "src/chain_c.py", "--budget", "1k"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let markdown = String::from_utf8(output).unwrap();
    assert!(contextcut::tokens::count_budget(&markdown) <= 1000);
    assert!(markdown.contains("## src/chain_c.py"));
    assert!(!markdown.contains("## src/chain_b.py"));
    assert!(markdown.contains("## src/chain_a.py")); // Smaller, more distant candidate still fits.
    assert!(markdown.contains("1 related/candidate files omitted"));
}

#[test]
fn budget_failure_does_not_overwrite_output_or_copy() {
    let repo = setup_git();
    let output = repo.path().join("saved.md");
    fs::write(&output, "KEEP_EXISTING_OUTPUT").unwrap();
    fs::write(
        repo.path().join("src/chain_c.py"),
        "CHANGED = 1\n".repeat(500),
    )
    .unwrap();
    contextcut()
        .arg("review")
        .arg(repo.path())
        .args(["--budget", "1", "--copy", "-o"])
        .arg(&output)
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("increase --budget"));
    assert_eq!(fs::read_to_string(output).unwrap(), "KEEP_EXISTING_OUTPUT");
}

#[test]
fn budget_is_enforced_for_review_including_patch_and_map() {
    let repo = setup_git();
    fs::write(repo.path().join("src/chain_c.py"), "CHANGE = 2\n").unwrap();
    let output = contextcut()
        .arg("review")
        .arg(repo.path())
        .args(["--budget", "2k"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("+CHANGE = 2"));
    assert!(text.contains("## Dependency map"));
    assert!(contextcut::tokens::count_budget(&text) <= 2000);
}

#[test]
fn existing_output_is_not_repacked() {
    let repo = setup();
    let output = repo.path().join("packed.md");
    fs::write(&output, "OLD_PACKED_CONTENT").unwrap();
    contextcut()
        .arg(repo.path())
        .arg("-o")
        .arg(&output)
        .assert()
        .success();
    assert!(
        !fs::read_to_string(output)
            .unwrap()
            .contains("OLD_PACKED_CONTENT")
    );
}

#[test]
fn invalid_budgets_and_conflicting_copy_dry_run_are_rejected() {
    for budget in ["0", "-1", "nonsense", "18446744073709551615k"] {
        contextcut().args(["--budget", budget]).assert().failure();
    }
    contextcut()
        .args(["review", "--copy", "--tokens-only"])
        .assert()
        .failure();
}

#[cfg(unix)]
#[test]
fn clipboard_receives_exact_output_and_failure_is_actionable() {
    use std::os::unix::fs::PermissionsExt;
    let repo = setup();
    let helpers = tempfile::tempdir().unwrap();
    let captured = helpers.path().join("clipboard.txt");
    let program = if cfg!(target_os = "macos") {
        "pbcopy"
    } else {
        "wl-copy"
    };
    let helper = helpers.path().join(program);
    fs::write(&helper, "#!/bin/sh\n/bin/cat > \"$TEST_CLIPBOARD\"\n").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
    let expected = contextcut()
        .arg(repo.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    contextcut()
        .arg(repo.path())
        .arg("--copy")
        .env("PATH", helpers.path())
        .env("TEST_CLIPBOARD", &captured)
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Copied"));
    assert_eq!(fs::read(&captured).unwrap(), expected);
    fs::write(&helper, "#!/bin/sh\nexit 1\n").unwrap();
    contextcut()
        .arg(repo.path())
        .arg("--copy")
        .env("PATH", helpers.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("use -o packed.md instead"));
}

#[test]
fn review_rename_includes_old_and_new_paths() {
    let repo = setup_git();
    git(
        repo.path(),
        &["mv", "src/chain_c.py", "src/renamed leaf.py"],
    );
    contextcut()
        .arg("review")
        .arg(repo.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("deleted file mode")
                .and(predicate::str::contains("new file mode"))
                .and(predicate::str::contains("## src/renamed leaf.py"))
                .and(predicate::str::contains("## src/chain_b.py")),
        );
}

#[cfg(unix)]
#[test]
fn diff_handles_tracked_filenames_with_newlines() {
    let repo = setup_git();
    let name = "line\nbreak.py";
    fs::write(repo.path().join(name), "OLD = 1\n").unwrap();
    git(repo.path(), &["add", name]);
    git(
        repo.path(),
        &[
            "-c",
            "user.name=test",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "odd name",
        ],
    );
    fs::write(repo.path().join(name), "NEWLINE_PATH_MARKER = 1\n").unwrap();
    contextcut()
        .arg(repo.path())
        .arg("--diff")
        .assert()
        .success()
        .stdout(predicate::str::contains("NEWLINE_PATH_MARKER"));
}

#[test]
fn review_accepts_global_options_before_the_command() {
    let repo = setup_git();
    fs::write(repo.path().join("new.py"), "NEW = 1\n").unwrap();
    contextcut()
        .args(["--budget", "2k", "--tokens-only", "review"])
        .arg(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("/ 2000 o200k_base tokens"));
    contextcut()
        .arg(repo.path())
        .arg("review")
        .assert()
        .failure()
        .stderr(predicate::str::contains("put the repository path after"));
}
