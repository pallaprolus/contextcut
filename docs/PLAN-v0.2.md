# ContextCut v0.2 — Build Plan

v0.1 shipped the commodity layer (pack + prune + token estimates) — table stakes shared with repomix/code2prompt. v0.2 ships the differentiators: **dependency-aware packing** (pack only the blast radius of the files you're changing) and **exact Claude token counts** via Anthropic's count-tokens API, plus distribution (CI + crates.io). Facts verified up front: the crate name `contextcut` is unclaimed on crates.io, and the count-tokens endpoint is `POST /v1/messages/count_tokens` (free to call, needs `ANTHROPIC_API_KEY`).

Build order is by value-per-hour: distribution first (small, compounding), then the dependency graph (the headline feature), then diff mode and exact tokens stacked on top of it.

## M0 — Distribution: CI + crates.io (~1h)

- `.github/workflows/ci.yml`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on push/PR (ubuntu + macos matrix).
- `cargo publish` (Cargo.toml metadata is already complete: license, description, keywords, categories).
- README: CI badge + install becomes `cargo install contextcut`.

**Gate:** green Actions run on the repo; `cargo install contextcut` works from a clean machine/temp CARGO_HOME.

## M1 — Dependency graph + `--related` (the differentiator, ~half day)

New module `src/deps.rs`: lightweight, line-based import extraction (no tree-sitter — same pragmatism as `strip.rs`):

| Language | Patterns |
|---|---|
| Python | `import x.y`, `from x.y import z` → resolve to `x/y.py` or `x/y/__init__.py` relative to root/source dir |
| JS/TS | `import ... from './x'`, `require('./x')` → resolve `./x.{js,ts,tsx,jsx}` and `/index.*`; ignore bare package specifiers |
| Rust | `mod x;`, `use crate::x` → `x.rs` / `x/mod.rs` |
| Go | same-module imports from the `import (...)` block (module path read from `go.mod`) |

Graph = forward edges (file → its imports) + reverse edges (file → its importers), built only over the already-walked file list, so it inherits all v0.1 pruning for free.

**CLI:**
- `--related <PATH>` (repeatable): pack only files reachable from PATH in the graph — forward *and* reverse — up to `--depth <N>` (default 2).
- `--map`: append a "## Dependency map" section to the Markdown header showing the tree for packed files.

**Testing (gates):**
- Unit tests per extractor with trap cases: `"import os"` inside a string literal, commented-out imports, relative `from . import`, `require` in a template literal.
- Fixture: extend `tests/fixtures/mini-repo` with an import chain (`a.py → b.py → c.py`, plus an unrelated `d.py`); integration test asserts `--related a.py --depth 1` packs a+b but not c/d, depth 2 adds c.
- Dogfood: `contextcut ../kube-foresight --related <one core module> --tokens-only` → token count must drop well below the full-repo 101k, and the file list must eyeball as sensible.

## M2 — Diff mode: `--diff [REF]` (~2h, builds on M1)

- `--diff` packs files changed vs REF (default `HEAD`; use `git diff --name-only <REF>` plus untracked via `git ls-files --others --exclude-standard`) **plus their reverse-dependency blast radius** from the M1 graph (`--depth` applies).
- Degrades gracefully outside a git repo: clear error, exit 1.

**Testing:** integration test builds a tempdir git repo (reuse the fixture-copy helper), commits, modifies one file, asserts `--diff` packs the changed file + its importers only. Dogfood: touch one kube-foresight file, check packed set.

## M3 — Exact Claude tokens: `--exact-claude` (~2h)

Anthropic publishes no local tokenizer, but the count-tokens endpoint gives exact numbers. Opt-in flag (tool stays offline-first by default):

- New dep: `ureq` (sync HTTP, no tokio — preserves the no-async rule) + `serde`/`serde_json` for the body.
- `POST https://api.anthropic.com/v1/messages/count_tokens`, headers `x-api-key: $ANTHROPIC_API_KEY`, `anthropic-version: 2023-06-01`, body `{"model": "claude-opus-4-8", "messages": [{"role": "user", "content": <markdown>}]}` → `input_tokens`.
- Table shows `Claude (exact)` replacing the approx row; on any error (no key, network, 4xx) print a one-line warning to stderr and fall back to the approximation — never fail the pack.
- Side quest while here: compare exact vs `cl100k × 1.15` on /tmp/kf.md once and adjust the constant if it's off by more than ~10% (Anthropic's own docs say tiktoken undercounts Claude by ~15–20% on text, more on code — the current factor may be conservative for code).

**Testing:** unit test the request-body construction (no network); one `#[ignore]`-by-default integration test that runs only when `ANTHROPIC_API_KEY` is set; manual gate comparing exact vs approx on kube-foresight.

## M4 — Release v0.2.0 (~30min)

CHANGELOG, README feature docs (dependency mapping gets the top spot — it's the differentiator), `cargo publish`, tag + GitHub Release.

## Deferred to v0.3
- tree-sitter comment stripping (replaces line-based strip.rs)
- `--map`-only mode as architecture overview without file bodies
- Homebrew tap
- Gemini count-tokens API

## Carry-over conventions from v0.1
Sorted walker output (snapshot stability), fence-collision handling, char-boundary safety, lib/bin split, every milestone gated by a runnable test, dogfood on kube-foresight before calling anything done.
