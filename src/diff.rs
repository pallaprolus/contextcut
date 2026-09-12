use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Resolve before using a revision, preventing option or path ambiguity.
pub fn resolve(root: &Path, reference: &str) -> Result<String> {
    let rev = format!("{reference}^{{commit}}");
    Ok(
        git(root, &["rev-parse", "--verify", "--end-of-options", &rev])?
            .trim()
            .into(),
    )
}

pub fn changed_files(root: &Path, reference: &str) -> Result<Vec<PathBuf>> {
    let revision = resolve(root, reference)?;
    let mut paths = tracked_changes(root, &revision)?;
    paths.extend(untracked(root)?);
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub fn tracked_changes(root: &Path, revision: &str) -> Result<Vec<PathBuf>> {
    paths(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--name-only",
            "-z",
            "--relative",
            revision,
            "--",
            ".",
        ],
    )
}

pub fn deleted_files(root: &Path, revision: &str) -> Result<Vec<PathBuf>> {
    paths(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--name-only",
            "--diff-filter=D",
            "-z",
            "--relative",
            revision,
            "--",
            ".",
        ],
    )
}

pub fn untracked(root: &Path) -> Result<Vec<PathBuf>> {
    paths(
        root,
        &[
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--",
            ".",
        ],
    )
}

fn paths(root: &Path, args: &[&str]) -> Result<Vec<PathBuf>> {
    let bytes = git_bytes(root, args)?;
    bytes
        .split(|b| *b == 0)
        .filter(|b| !b.is_empty())
        .map(|b| {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt;
                Ok(PathBuf::from(std::ffi::OsString::from_vec(b.to_vec())))
            }
            #[cfg(not(unix))]
            {
                Ok(PathBuf::from(
                    std::str::from_utf8(b).context("non-UTF-8 git path")?,
                ))
            }
        })
        .collect()
}

pub fn original(root: &Path, revision: &str, path: &Path) -> Result<Vec<u8>> {
    let prefix = git(root, &["rev-parse", "--show-prefix"])?;
    let path = path
        .to_str()
        .context("review of deleted non-UTF-8 paths is unsupported")?;
    git_bytes(
        root,
        &[
            "show",
            &format!("{revision}:{}{path}", prefix.trim_end_matches('\n')),
        ],
    )
}

/// Only eligible tracked paths enter the patch; untracked bodies are rendered separately.
pub fn patch(root: &Path, revision: &str, selected: &[PathBuf]) -> Result<String> {
    // No path arguments would accidentally select the entire repository.
    if selected.is_empty() {
        return Ok(String::new());
    }
    let mut cmd = command(root);
    cmd.args([
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--no-renames",
        "--relative",
        revision,
        "--",
    ])
    .args(selected);
    output(cmd)
}

pub fn is_ignored(root: &Path, path: &Path) -> Result<bool> {
    let output = command(root)
        .env_remove("GIT_LITERAL_PATHSPECS")
        .args(["check-ignore", "--no-index", "-q", "--"])
        .arg(path)
        .output()?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!(
            "git check-ignore failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

fn command(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root).env("GIT_LITERAL_PATHSPECS", "1");
    cmd
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let mut cmd = command(root);
    cmd.args(args);
    output(cmd)
}

fn output(mut cmd: Command) -> Result<String> {
    let output = cmd.output().context("running git (is it installed?)")?;
    if !output.status.success() {
        bail!(
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = command(root)
        .args(args)
        .output()
        .context("running git (is it installed?)")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}
