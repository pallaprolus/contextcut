# Changelog

## 0.3.1 — 2026-09-30

### Fixed
- `--diff` and `review` work in a repository with no commits yet: every file is treated as an addition. An unknown revision now reports `unknown git revision '<ref>'`.
- A directory that is not inside a Git repository now honors its own `.gitignore` (ancestor ignore files outside a repository are not applied).
- Write errors on stdout now fail the run; a closed pipe is still ignored.
- Oversized files are read only up to the size cap; the truncation marker reports the full file size.
- Traversal errors (for example, unreadable subdirectories) count toward the `unreadable` total instead of being dropped.
- `review --strip-comments` strips deleted-file bodies too.
- Code fences grow past backtick runs indented by up to three spaces, so file content cannot close its fence early.
- A NUL byte anywhere in a file marks it binary, not only within the first 1 KiB.
- Import graph: Rust `mod x;` inside `foo.rs` resolves to `foo/x.rs`; Python `from pkg import sub` resolves `pkg/sub/__init__.py`; literal JS/TS `import('./x')` is recognized; Go imports still resolve when `go.mod` is filtered out of the output.

## 0.3.0 — 2026-09-12

### Added
- Prebuilt macOS (Apple Silicon/Intel), Linux (ARM64/x86-64 musl), and Windows (x86-64) archives with SHA-256 checksums and per-platform smoke tests.
- Homebrew installation through `pallaprolus/tap/contextcut`, using the prebuilt archives.
- A tag-triggered workflow that prepares draft GitHub releases and generates the Homebrew formula from actual archive hashes.
- `contextcut review [PATH] --base REF`: an AI review packet containing the actual patch, changed file bodies, an import map, and inclusion reasons. Includes staged changes, untracked additions, and deleted file context.
- `--budget 20k`: fit complete rendered output to an o200k_base token budget. Review defaults to 20k; selected/changed files and the patch remain required. Report omitted context and fail before output if required content does not fit.
- `--copy`: clipboard delivery on macOS, Windows, Wayland, and X11, with file-output guidance when unavailable.

### Fixed
- Explicit Cargo package allowlist excludes local launch drafts and development metadata.
- Do not include a previous output file when regenerating it.
- Parse Git filenames with NUL delimiters and resolve revisions before using them; keep diffs within the selected directory and disable external diff/text conversion commands.
- Reuse local tokenizers across repeated budget checks.
- Reject overflowing file-size inputs.

## 0.2.1 — 2026-06-11

### Fixed
- Python import resolution now handles the `src/` layout: absolute imports
  like `from pkg.client import X` resolve to `src/pkg/client.py`, so
  test→code edges appear in src-layout projects. Found dogfooding on a
  real MCP server repo.

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
