# Distribution maintenance

`.github/workflows/release.yml` builds and tests five native targets on pushes to `main` and version tags. Only tag builds create a draft release. It packages only the binary, README, and license, tests each extracted archive in a temporary repository, then creates a **draft** GitHub release with archives, `SHA256SUMS`, and a generated `contextcut.rb` formula. It never changes an already-published release.

The Homebrew tap is [pallaprolus/homebrew-tap](https://github.com/pallaprolus/homebrew-tap). Its formula selects the matching macOS/Linux archive by OS and CPU. No Rust compiler is needed for binary installs.

## Cut a release

1. Bump `Cargo.toml` and `Cargo.lock`, finalize `CHANGELOG.md`, and add `docs/releases/vX.Y.Z.md`. Update installation examples to that version.
2. Run tests, clippy, formatting, `cargo package`, and the repository's release checks. Inspect the crate's file list so local drafts are not published.
3. Commit and push the release changes, then tag that exact commit as `vX.Y.Z` and push the tag. The workflow refuses tags that do not match the Cargo version.
4. Wait for all five builds and archive smoke tests to pass. Inspect the draft's seven assets: five archives, `SHA256SUMS`, and `contextcut.rb`. To retry a failed draft build, re-run the workflow on the same tag; published assets are never overwritten.
5. Download the draft assets with `gh release download vX.Y.Z --dir <temporary-directory>`. Verify checksums and inspect the generated Homebrew formula.
6. Publish the draft: `gh release edit vX.Y.Z --draft=false --latest`.
7. Copy the released `contextcut.rb` to `Formula/contextcut.rb` in the Homebrew tap, commit, and push it. Run the tap's install/test workflow; it exercises the public download URLs on macOS and Linux.

Publishing the tap is an explicit maintainer step using normal GitHub access; no cross-repository token is stored in the ContextCut workflow. A crates.io release is separate from GitHub binaries/Homebrew and requires `cargo publish`. Until that is done, use the documented Git-tag Cargo installation command for the same version.

## Local archive smoke test

```bash
cargo build --release --locked
python3 scripts/release.py package --target aarch64-apple-darwin --binary target/release/contextcut
python3 scripts/release.py smoke dist/contextcut-0.3.0-aarch64-apple-darwin.tar.gz
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Use the native target name on other hosts. `scripts/release.py` requires Python 3.12 or newer. Generating the manifest requires all five real archives; do not use placeholder hashes. Binary release jobs run `cargo test` for each target before packaging.
