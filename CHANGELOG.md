# Changelog

## 0.2.0 — 2026-06-11

### Added
- **Dependency-aware packing**: `--related <PATH>` packs only files connected
  to PATH in the import graph (imports and importers), up to `--depth <N>`
  hops (default 2). Import extraction covers Python, JS/TS, Rust, and Go;
  specifiers are resolved against the packed file set, which filters
  false positives from the line-based extraction.
- **Diff mode**: `--diff [REF]` packs files changed vs REF (default `HEAD`)
  plus untracked files, expanded by their import blast radius.
- **Dependency map**: `--map` appends an imports/importers section to the
  output.
- **Exact Claude counts**: `--exact-claude` queries Anthropic's count-tokens
  API (requires `ANTHROPIC_API_KEY`); falls back to the ×1.15 approximation
  with a warning on any error.
- CI: GitHub Actions running fmt, clippy (`-D warnings`), and tests on
  Ubuntu and macOS.

## 0.1.0 — 2026-06-11

- Initial release: gitignore-aware walking, always-on noise pruning
  (binaries, lockfiles, minified assets, vendor/cache dirs), file-tree
  header + language-tagged fenced blocks with fence-collision handling,
  size-cap truncation, `--strip-comments`, `--include`/`--exclude` globs,
  `--tokens-only`, and a per-provider token estimate table on stderr.
