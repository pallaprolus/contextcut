pub mod cli;
pub mod pruner;
pub mod renderer;
pub mod strip;
pub mod tokens;
pub mod walker;

use std::fs;
use std::io::Write;

use anyhow::{Context, Result, bail};

use cli::Cli;
use pruner::{FileDecision, SkipReason};

/// Run the full pack pipeline: walk → prune → (strip) → render → count → emit.
pub fn run(cli: &Cli) -> Result<()> {
    if !cli.path.is_dir() {
        bail!("{} is not a directory", cli.path.display());
    }

    let paths = walker::walk(&cli.path, cli.no_gitignore)?;
    let filters = pruner::Filters::new(&cli.include, &cli.exclude)?;

    let mut packed: Vec<renderer::PackedFile> = Vec::new();
    let mut stats = Stats::default();

    for path in &paths {
        let rel = path.strip_prefix(&cli.path).unwrap_or(path);
        match pruner::decide(path, rel, &filters, cli.max_file_size) {
            FileDecision::Keep(content) | FileDecision::Truncated(content) => {
                let content = if cli.strip_comments {
                    strip::strip_comments(rel, &content)
                } else {
                    content
                };
                packed.push(renderer::PackedFile {
                    rel_path: rel.to_path_buf(),
                    content,
                });
            }
            FileDecision::Skip(reason) => stats.count_skip(reason),
        }
    }
    stats.packed = packed.len();

    let markdown = renderer::render(&cli.path, &packed);
    let estimate = tokens::estimate(&markdown);

    if !cli.tokens_only {
        match &cli.output {
            Some(file) => fs::write(file, &markdown)
                .with_context(|| format!("writing {}", file.display()))?,
            None => {
                // Locked stdout write; ignore EPIPE-style failures gracefully.
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(markdown.as_bytes());
            }
        }
    }

    eprintln!("{}", stats.summary(markdown.len()));
    eprintln!("{}", estimate.table());
    Ok(())
}

/// Counters for the stderr summary line.
#[derive(Default)]
struct Stats {
    packed: usize,
    ignored: usize,
    binary: usize,
    noise: usize,
    filtered: usize,
}

impl Stats {
    fn count_skip(&mut self, reason: SkipReason) {
        match reason {
            SkipReason::Binary => self.binary += 1,
            SkipReason::Lockfile | SkipReason::Minified | SkipReason::Vendor => self.noise += 1,
            SkipReason::Filtered => self.filtered += 1,
            SkipReason::Unreadable => self.ignored += 1,
        }
    }

    fn summary(&self, bytes: usize) -> String {
        format!(
            "  Files packed:  {}   (skipped: {} binary, {} lockfile/minified/vendor, {} filtered, {} unreadable)\n  Output size:   {}",
            self.packed,
            self.binary,
            self.noise,
            self.filtered,
            self.ignored,
            human_bytes(bytes)
        )
    }
}

fn human_bytes(n: usize) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}
