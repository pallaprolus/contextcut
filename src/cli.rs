use std::path::PathBuf;

use clap::Parser;

/// Pack a repository into ultra-dense, AI-optimized Markdown.
///
/// Walks PATH respecting .gitignore, prunes noise (binaries, lockfiles,
/// minified assets, vendor dirs), and emits one Markdown document with a
/// file tree header and language-tagged code blocks. A token-estimate
/// summary is printed to stderr so stdout stays pipeable.
#[derive(Parser, Debug)]
#[command(name = "contextcut", version, about)]
pub struct Cli {
    /// Root directory to pack
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Write Markdown to a file instead of stdout
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Strip full-line comments for supported languages
    /// (py, rs, js/ts, go, c/cpp, java, sh, yaml/toml)
    #[arg(long)]
    pub strip_comments: bool,

    /// Max bytes per file before truncation; accepts suffixes like 64kb, 1mb
    #[arg(long, value_name = "SIZE", default_value = "64kb", value_parser = parse_size)]
    pub max_file_size: u64,

    /// Only pack files matching this glob (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// Skip files matching this glob (repeatable, applied after includes)
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Pack only files related to PATH in the import graph (repeatable);
    /// follows imports and importers up to --depth hops
    #[arg(long, value_name = "PATH")]
    pub related: Vec<PathBuf>,

    /// Pack files changed vs REF (default HEAD) plus untracked files and
    /// their import-graph blast radius up to --depth hops
    #[arg(long, value_name = "REF", num_args = 0..=1, default_missing_value = "HEAD")]
    pub diff: Option<String>,

    /// Hops to follow in the import graph for --related and --diff
    #[arg(long, value_name = "N", default_value_t = 2)]
    pub depth: usize,

    /// Include a dependency map section (imports → / importers ←) in the output
    #[arg(long)]
    pub map: bool,

    /// Get the exact Claude token count from Anthropic's count-tokens API
    /// (requires ANTHROPIC_API_KEY; falls back to the approximation on error)
    #[arg(long)]
    pub exact_claude: bool,

    /// Dry run: print only stats and the token table, emit no Markdown
    #[arg(long)]
    pub tokens_only: bool,

    /// Ignore .gitignore files (built-in prunes still apply)
    #[arg(long)]
    pub no_gitignore: bool,
}

/// Parse "64kb" / "1mb" / "4096" into bytes.
fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim().to_lowercase();
    let (num, mult) = if let Some(n) = s.strip_suffix("mb") {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("kb") {
        (n, 1024)
    } else if let Some(n) = s.strip_suffix('b') {
        (n, 1)
    } else {
        (s.as_str(), 1)
    };
    num.trim()
        .parse::<u64>()
        .map(|n| n * mult)
        .map_err(|_| format!("invalid size: {s:?} (expected e.g. 4096, 64kb, 1mb)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_bytes() {
        assert_eq!(parse_size("4096").unwrap(), 4096);
    }

    #[test]
    fn parses_kb_and_mb() {
        assert_eq!(parse_size("64kb").unwrap(), 65536);
        assert_eq!(parse_size("1mb").unwrap(), 1048576);
        assert_eq!(parse_size("2MB").unwrap(), 2 * 1024 * 1024);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_size("lots").is_err());
    }
}
