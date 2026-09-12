# ContextCut

[![CI](https://github.com/pallaprolus/contextcut/actions/workflows/ci.yml/badge.svg)](https://github.com/pallaprolus/contextcut/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/contextcut.svg)](https://crates.io/crates/contextcut)

**Get the code you need into an AI conversation in one command.**

ContextCut prepares a change review with the actual Git patch, changed files, and nearby code and tests found through imports. It fits the result into a token budget and can copy it straight to your clipboard. It also packs whole repositories or selected modules as Markdown, pruning ignored files, binaries, lockfiles, and vendor directories.

### Review a change

```bash
# Uncommitted work, including staged changes and untracked additions:
contextcut review --copy

# Compare main directly with your working tree:
contextcut review --base main --budget 20k --copy

# Save a review packet, or preview its token usage:
contextcut review -o review.md
contextcut review --tokens-only
```

Paste the result into your AI chat. Review mode includes a review instruction, the patch, an import map, and a list explaining why each file was included. Deleted files are labeled as base revision content; untracked additions appear as current file bodies. Only changes that pass the normal pruning and filters enter the packet.

The default review budget is **20,000 o200k_base tokens**, including the patch and all Markdown metadata. Changed file bodies (subject to `--max-file-size`) and the complete eligible patch are required. Nearest import neighbors are added first, with path order breaking ties; oversized optional files are skipped so smaller ones can still fit. If required content exceeds the budget, ContextCut fails before writing or copying and asks for a larger budget or narrower selection. Other models can count differently.

`--base` defaults to `HEAD`. Comparison is directly against that revision, **not the branch merge base**. For branch-only changes after branches diverge, pass `--base "$(git merge-base main HEAD)"`. Import extraction is heuristic: connected tests may be found, but this does not guarantee every affected file or test is included.

### Pack a module or repository

```bash
contextcut . --related src/api.py --depth 1 --budget 10k --copy
contextcut ~/code/my-project -o packed.md
```

Without `review`, the existing packing behavior and stdout piping remain available. `--budget` is optional; when set, explicitly selected/changed files are required and other candidate files are added while they fit. For whole-repository packing without seeds, candidates are considered in path order.

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

Install on macOS or Linux with Homebrew; no Rust compiler is needed:

```bash
brew install pallaprolus/tap/contextcut
```

Or download a [prebuilt binary](https://github.com/pallaprolus/contextcut/releases/latest):

| Platform | Archive target |
|---|---|
| macOS Apple Silicon (11+) | `aarch64-apple-darwin` |
| macOS Intel (11+) | `x86_64-apple-darwin` |
| Linux ARM64 | `aarch64-unknown-linux-musl` |
| Linux x86-64 | `x86_64-unknown-linux-musl` |
| Windows x86-64 | `x86_64-pc-windows-msvc` |

Archives are named `contextcut-<version>-<target>.tar.gz` (`.zip` on Windows). Compare the archive's SHA-256 with the release's `SHA256SUMS`, extract it, and put `contextcut` (or `contextcut.exe`) on your PATH. Linux binaries use musl and do not require glibc. Git must be installed for `review` and `--diff`.

For example, after downloading the macOS Apple Silicon archive and checksum file:

```bash
shasum -a 256 contextcut-0.3.0-aarch64-apple-darwin.tar.gz
# Compare with its entry in SHA256SUMS, then extract:
tar -xzf contextcut-0.3.0-aarch64-apple-darwin.tar.gz
mkdir -p "$HOME/.local/bin"
install -m 755 contextcut "$HOME/.local/bin/contextcut"
# Add $HOME/.local/bin to your shell's PATH if it is not already there.
```

To build the same version with Rust:

```bash
cargo install --git https://github.com/pallaprolus/contextcut --tag v0.3.0 --locked
# Or from a checkout:
cargo install --path . --locked
```

The existing crates.io release is v0.2.1; `cargo install contextcut` does not yet include the newer review, budget, and clipboard features. GitHub binaries and Homebrew provide v0.3.0.

`--copy` uses `pbcopy` on macOS, PowerShell on Windows, and `wl-copy`, `xclip`, or `xsel` on Linux. On a headless machine or if no clipboard helper is available, use `-o packed.md`. `--copy -o packed.md` both saves and copies; the saved file remains available if copying fails. `--copy` cannot be combined with `--tokens-only`.

## Usage

```bash
contextcut [PATH] [OPTIONS]
contextcut review [PATH] [--base REF] [OPTIONS]
```

| Flag | Default | Effect |
|---|---|---|
| `--copy` | off | Copy Markdown to the clipboard; suppress Markdown on stdout |
| `--budget <N>` | unlimited; `20k` for review | Cap complete output using o200k_base; accepts `20000` or `20k` |
| `PATH` | `.` | Root directory to pack |
| `-o, --output <FILE>` | stdout | Write Markdown to a file (the stats table always goes to stderr, so stdout stays pipeable) |
| `--related <PATH>` | — | Pack only files related to PATH in the import graph (repeatable): its imports *and* its importers |
| `--diff [REF]` | — | Pack files changed vs REF (default `HEAD`) plus untracked files, with their import blast radius |
| `--depth <N>` | `2` | Hops to follow in the import graph for `--related`/`--diff` |
| `--map` | off; on for review | Append a dependency map section (`→` imports, `←` importers) to the output |
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

- **Local tokenizer counts are exact for the named encodings** — real BPE via [`tiktoken-rs`](https://crates.io/crates/tiktoken-rs) (`o200k_base` and `cl100k_base`). These count the generated Markdown, not a complete API request or every model’s billing tokens. Verified against Python `tiktoken`.
- **Claude is exact with `--exact-claude`** — Anthropic publishes no local tokenizer, but their count-tokens API returns exact numbers (free to call; set `ANTHROPIC_API_KEY`). Without the flag (or on any API error) we report `cl100k × 1.15` as a rough budgeting factor, labeled "approx".
- **Gemini is an approximation** — we reuse the `o200k_base` count as a nearby proxy, labeled "approx".
- Special tokens (a literal `<|endoftext|>` in source) are counted as plain text, never as control tokens.

## Known limitations

- `--strip-comments` is line-based: it removes *full-line* comments only and leaves inline trailing comments. Rare multi-line strings whose lines begin with `#`/`//` could be affected. A tree-sitter-based stripper is planned.
- Non-UTF-8 text files are lossy-converted (`U+FFFD` replacement) rather than skipped.
- Claude/Gemini counts are estimates — treat them as budgeting guidance, not billing truth.

## Roadmap

- **Tree-sitter comment stripping**: replaces the line-based stripper
- **Architecture overview mode**: `--map` without file bodies
- Gemini count-tokens API

## Development

See [distribution maintenance](docs/DISTRIBUTION.md) for binary packaging, checksums, and Homebrew updates.

```bash
cargo test            # unit + fixture-based integration + insta snapshot tests
cargo insta review    # review Markdown-format snapshot changes
cargo clippy          # lint (CI gate)
```

Integration tests run the real binary against `tests/fixtures/mini-repo/`, a planted-noise fixture (gitignored secrets, a lockfile, a minified asset, a real PNG, comment/string traps). The fixture's `gitignore.txt` is renamed to `.gitignore` inside a tempdir at test time so it behaves identically regardless of the host repo's git context.

## License

MIT
