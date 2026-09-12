pub mod cli;
pub mod clipboard;
pub mod deps;
pub mod diff;
pub mod exact;
pub mod pruner;
pub mod renderer;
pub mod review;
pub mod selection;
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
    let root = cli.root();
    if cli.is_review() && cli.path != std::path::Path::new(".") {
        bail!("put the repository path after the command: contextcut review <PATH>");
    }
    if cli.is_review() && (cli.diff.is_some() || !cli.related.is_empty()) {
        bail!("review uses --base; --diff and --related are for the pack command");
    }
    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }

    let paths = walker::walk(root, cli.no_gitignore)?;
    let filters = pruner::Filters::new(&cli.include, &cli.exclude)?;

    let mut packed: Vec<renderer::PackedFile> = Vec::new();
    let mut stats = Stats::default();

    let output_path = cli.output.as_ref().and_then(|p| p.canonicalize().ok());
    for path in &paths {
        if output_path
            .as_ref()
            .is_some_and(|out| path.canonicalize().ok().as_ref() == Some(out))
        {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path);
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
    let review = if cli.is_review() {
        Some(review::Review::prepare(cli, &mut packed, &filters)?)
    } else {
        None
    };
    // Import graph: needed for --related/--diff filtering and --map.
    let graph = if cli.map || !cli.related.is_empty() || cli.reference().is_some() {
        let entries: Vec<(std::path::PathBuf, String)> = packed
            .iter()
            .map(|f| (f.rel_path.clone(), f.content.clone()))
            .collect();
        Some(deps::Graph::build(&entries))
    } else {
        None
    };

    let mut seeds = Vec::new();
    for seed in &cli.related {
        let rel = seed.strip_prefix(root).unwrap_or(seed).to_path_buf();
        if !packed.iter().any(|f| f.rel_path == rel) {
            bail!("--related {}: no packed file matches", rel.display());
        }
        seeds.push(rel);
    }
    if let Some(review) = &review {
        if review.changed.is_empty() {
            bail!("no packable changed files; use --base <revision> to review committed changes");
        }
        seeds.extend(review.changed.iter().cloned());
    } else if let Some(reference) = &cli.diff {
        let changed = diff::changed_files(root, reference)?;
        let packable: Vec<_> = changed
            .into_iter()
            .filter(|c| packed.iter().any(|f| f.rel_path == *c))
            .collect();
        if packable.is_empty() {
            bail!("--diff {reference}: no packable changed files");
        }
        seeds.extend(packable);
    }
    let distances = graph
        .as_ref()
        .map(|g| g.distances(&seeds, cli.depth))
        .unwrap_or_default();
    if !seeds.is_empty() {
        packed.retain(|f| distances.contains_key(&f.rel_path));
    }
    let selection = selection::Selection {
        root,
        graph: graph.as_ref(),
        map: cli.map || cli.is_review(),
        distances,
        review: review.as_ref(),
        budget: cli.token_budget(),
    };
    let (markdown, count, omitted) = selection.fit(packed)?;
    stats.packed = count;
    let estimate = tokens::estimate(&markdown);
    let exact_claude = if cli.exact_claude {
        match exact::count(&markdown) {
            Ok(n) => Some(n),
            Err(err) => {
                eprintln!(
                    "  warning: exact Claude count unavailable ({err:#}); falling back to approximation"
                );
                None
            }
        }
    } else {
        None
    };

    if !cli.tokens_only {
        match &cli.output {
            Some(file) => {
                fs::write(file, &markdown).with_context(|| format!("writing {}", file.display()))?
            }
            None if cli.copy => {}
            None => {
                // Locked stdout write; ignore EPIPE-style failures gracefully.
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(markdown.as_bytes());
            }
        }
        if cli.copy {
            clipboard::copy(&markdown)?;
            eprintln!("  Copied review/context to clipboard. Paste it into your AI chat.");
        }
    }
    if cli.token_budget().is_some() {
        eprintln!(
            "  Budget: {} / {} o200k_base tokens; {omitted} files omitted",
            estimate.o200k,
            cli.token_budget().unwrap()
        );
    }

    eprintln!("{}", stats.summary(markdown.len()));
    eprintln!("{}", estimate.table(exact_claude));
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
