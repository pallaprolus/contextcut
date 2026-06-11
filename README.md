# ContextCut

[![CI](https://github.com/pallaprolus/contextcut/actions/workflows/ci.yml/badge.svg)](https://github.com/pallaprolus/contextcut/actions/workflows/ci.yml)

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
cargo install --git https://github.com/pallaprolus/contextcut
# (crates.io / Homebrew distribution planned)
```

## Usage

```bash
contextcut [PATH] [OPTIONS]
```

| Flag | Default | Effect |
|---|---|---|
| `PATH` | `.` | Root directory to pack |
| `-o, --output <FILE>` | stdout | Write Markdown to a file (the stats table always goes to stderr, so stdout stays pipeable) |
| `--tokens-only` | off | Dry run: stats + token table only, no Markdown |
| `--strip-comments` | off | Drop full-line comments (py, rs, js/ts, go, c/cpp, java, sh, yaml/toml) |
| `--max-file-size <SIZE>` | `64kb` | Truncate larger files with a `[truncated: N of M bytes]` marker (`4096`, `64kb`, `1mb`) |
| `--include <GLOB>` | all | Only pack matching files (repeatable), e.g. `--include '**/*.py'` |
| `--exclude <GLOB>` | none | Skip matching files (repeatable, applied after includes) |
| `--no-gitignore` | off | Ignore `.gitignore` rules (built-in prunes still apply) |

### What gets pruned automatically

No flags needed — this is the product's opinion:

- Anything matched by `.gitignore` / `.ignore` (via ripgrep's [`ignore`](https://crates.io/crates/ignore) walker)
- Binary files (content-sniffed, not extension-guessed)
- Lockfiles: `Cargo.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `poetry.lock`, `uv.lock`, `Pipfile.lock`, `Gemfile.lock`, `composer.lock`, `go.sum`, `flake.lock`
- Minified assets: `*.min.js`, `*.min.css`, `*.map`
- Vendor/cache dirs: `.git`, `node_modules`, `vendor`, `__pycache__`, `.venv`, `venv`, `dist`, `build`, `target`, `.pytest_cache`, `.ruff_cache`, `.mypy_cache`, `*.egg-info`, `.idea`, `.vscode`

## Token estimates: how they're computed

- **GPT counts are exact** — real BPE via [`tiktoken-rs`](https://crates.io/crates/tiktoken-rs) (`o200k_base` for GPT-4o/5-class, `cl100k_base` for GPT-4). Verified byte-identical against Python `tiktoken`.
- **Claude is an approximation** — Anthropic publishes no local tokenizer, so we report `cl100k × 1.15` as a rough budgeting factor (code tends to tokenize heavier on Claude). Treat it as guidance, not ground truth; exact counts via Anthropic's count-tokens API are planned for v0.2.
- **Gemini is an approximation** — we reuse the `o200k_base` count as a nearby proxy, labeled "approx".
- Special tokens (a literal `<|endoftext|>` in source) are counted as plain text, never as control tokens.

## Known limitations (v0.1)

- `--strip-comments` is line-based: it removes *full-line* comments only and leaves inline trailing comments. Rare multi-line strings whose lines begin with `#`/`//` could be affected. A tree-sitter-based stripper is planned for v0.2.
- Non-UTF-8 text files are lossy-converted (`U+FFFD` replacement) rather than skipped.
- Claude/Gemini counts are estimates — treat them as budgeting guidance, not billing truth.

## Roadmap

- **v0.2 — dependency mapping**: pack only files reachable from the paths you're changing, with an inline dependency tree
- **v0.2 — live token ticker**: provider count-token APIs for exact Claude/Gemini numbers
- **PR mode**: pack just a diff plus its blast radius

## Development

```bash
cargo test            # unit + fixture-based integration + insta snapshot tests
cargo insta review    # review Markdown-format snapshot changes
cargo clippy          # lint (CI gate)
```

Integration tests run the real binary against `tests/fixtures/mini-repo/`, a planted-noise fixture (gitignored secrets, a lockfile, a minified asset, a real PNG, comment/string traps). The fixture's `gitignore.txt` is renamed to `.gitignore` inside a tempdir at test time so it behaves identically regardless of the host repo's git context.

## License

MIT
