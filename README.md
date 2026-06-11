# ContextCut

[![CI](https://github.com/pallaprolus/contextcut/actions/workflows/ci.yml/badge.svg)](https://github.com/pallaprolus/contextcut/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/contextcut.svg)](https://crates.io/crates/contextcut)

**Pack a repository into ultra-dense, AI-optimized Markdown — with token estimates before you paste.**

Feeding a whole repo to an LLM wastes thousands of tokens on vendor directories, lockfiles, caches, and binaries. ContextCut walks your project gitignore-aware, prunes the noise, and emits one clean Markdown document (file tree + language-tagged code blocks) ready for any chat or agent context window — and tells you what it will cost in tokens *before* you send it.

```console
$ contextcut ~/code/my-project -o packed.md
  Files packed:  114   (skipped: 0 binary, 0 lockfile/minified/vendor, 0 filtered, 0 unreadable)
  Output size:   400.8 KB
  ── Estimated tokens ─────────────
  GPT (o200k_base)         101,052
  GPT-4 (cl100k_base)      100,147
  Claude (approx ×1.15)    115,169
  Gemini (approx)          101,052
```

Real-world result: a 2,240-file / 38 MB Python repo → 114 files / 0.4 MB of signal.

## Install

```bash
cargo install contextcut
# (Homebrew tap planned)
```

## Usage

```bash
contextcut [PATH] [OPTIONS]
```

| Flag | Default | Effect |
|---|---|---|
| `PATH` | `.` | Root directory to pack |
| `-o, --output <FILE>` | stdout | Write Markdown to a file (the stats table always goes to stderr, so stdout stays pipeable) |
| `--related <PATH>` | — | Pack only files related to PATH in the import graph (repeatable): its imports *and* its importers |
| `--diff [REF]` | — | Pack files changed vs REF (default `HEAD`) plus untracked files, with their import blast radius |
| `--depth <N>` | `2` | Hops to follow in the import graph for `--related`/`--diff` |
| `--map` | off | Append a dependency map section (`→` imports, `←` importers) to the output |
| `--exact-claude` | off | Exact Claude count via Anthropic's count-tokens API (needs `ANTHROPIC_API_KEY`; falls back to the approximation on any error) |
| `--tokens-only` | off | Dry run: stats + token table only, no Markdown |
| `--strip-comments` | off | Drop full-line comments (py, rs, js/ts, go, c/cpp, java, sh, yaml/toml) |
| `--max-file-size <SIZE>` | `64kb` | Truncate larger files with a `[truncated: N of M bytes]` marker (`4096`, `64kb`, `1mb`) |
| `--include <GLOB>` | all | Only pack matching files (repeatable), e.g. `--include '**/*.py'` |
| `--exclude <GLOB>` | none | Skip matching files (repeatable, applied after includes) |
| `--no-gitignore` | off | Ignore `.gitignore` rules (built-in prunes still apply) |

### Pack only the blast radius

Most questions are about *part* of a codebase. ContextCut builds an import graph (Python, JS/TS, Rust, Go — lightweight line-based extraction, resolved against the real file set) and packs only what's connected:

```bash
# Working on the scheduler? Pack it, what it imports, and what imports it:
contextcut . --related kube_foresight/scheduler.py --depth 1
#   → 5 files / ~5k tokens instead of 114 files / ~101k

# Reviewing a change? Pack the diff plus everything it can break:
contextcut . --diff main

# Add --map for an explicit imports/importers section the model can navigate by
contextcut . --related src/api.py --map
```

### What gets pruned automatically

No flags needed — this is the product's opinion:

- Anything matched by `.gitignore` / `.ignore` (via ripgrep's [`ignore`](https://crates.io/crates/ignore) walker)
- Binary files (content-sniffed, not extension-guessed)
- Lockfiles: `Cargo.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `poetry.lock`, `uv.lock`, `Pipfile.lock`, `Gemfile.lock`, `composer.lock`, `go.sum`, `flake.lock`
- Minified assets: `*.min.js`, `*.min.css`, `*.map`
- Vendor/cache dirs: `.git`, `node_modules`, `vendor`, `__pycache__`, `.venv`, `venv`, `dist`, `build`, `target`, `.pytest_cache`, `.ruff_cache`, `.mypy_cache`, `*.egg-info`, `.idea`, `.vscode`

## Token estimates: how they're computed

- **GPT counts are exact** — real BPE via [`tiktoken-rs`](https://crates.io/crates/tiktoken-rs) (`o200k_base` for GPT-4o/5-class, `cl100k_base` for GPT-4). Verified byte-identical against Python `tiktoken`.
- **Claude is exact with `--exact-claude`** — Anthropic publishes no local tokenizer, but their count-tokens API returns exact numbers (free to call; set `ANTHROPIC_API_KEY`). Without the flag (or on any API error) we report `cl100k × 1.15` as a rough budgeting factor, labeled "approx".
- **Gemini is an approximation** — we reuse the `o200k_base` count as a nearby proxy, labeled "approx".
- Special tokens (a literal `<|endoftext|>` in source) are counted as plain text, never as control tokens.

## Known limitations (v0.1)

- `--strip-comments` is line-based: it removes *full-line* comments only and leaves inline trailing comments. Rare multi-line strings whose lines begin with `#`/`//` could be affected. A tree-sitter-based stripper is planned for v0.2.
- Non-UTF-8 text files are lossy-converted (`U+FFFD` replacement) rather than skipped.
- Claude/Gemini counts are estimates — treat them as budgeting guidance, not billing truth.

## Roadmap

- **v0.3 — tree-sitter comment stripping**: replaces the line-based stripper
- **v0.3 — architecture overview mode**: `--map` without file bodies
- Homebrew tap; Gemini count-tokens API

## Development

```bash
cargo test            # unit + fixture-based integration + insta snapshot tests
cargo insta review    # review Markdown-format snapshot changes
cargo clippy          # lint (CI gate)
```

Integration tests run the real binary against `tests/fixtures/mini-repo/`, a planted-noise fixture (gitignored secrets, a lockfile, a minified asset, a real PNG, comment/string traps). The fixture's `gitignore.txt` is renamed to `.gitignore` inside a tempdir at test time so it behaves identically regardless of the host repo's git context.

## License

MIT
