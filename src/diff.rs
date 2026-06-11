use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Files changed vs `reference` plus untracked files, as paths relative to
/// `root`. `--relative` keeps git's output rooted at `root` even when it is
/// a subdirectory of the repository.
pub fn changed_files(root: &Path, reference: &str) -> Result<Vec<PathBuf>> {
    let diff = git(root, &["diff", "--name-only", "--relative", reference])?;
    let untracked = git(root, &["ls-files", "--others", "--exclude-standard"])?;

    Ok(diff
        .lines()
        .chain(untracked.lines())
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect())
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
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
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
