# GitHub Release Workflow

The release pipeline is defined in `.github/workflows/release.yml`.

## Trigger

The workflow runs on:

- tags matching `v*`

## What it does

1. Runs workspace tests on Linux.
2. Builds `drat` in release mode on native GitHub-hosted runners for:
   - Linux x86_64
   - Linux aarch64
   - macOS x86_64
   - macOS aarch64
   - Windows x86_64
3. Packages one archive per platform.
4. Verifies each archive with a smoke test:
   - `drat --version`
   - `drat run examples/hello.dt`
5. Generates per-archive `.sha256` files and a combined `SHA256SUMS.txt`.
6. Publishes the GitHub Release and uploads all assets.

## Archive Layout

Each release archive contains:

- `drat` or `drat.exe`
- the packaged Draton runtime static library
- `LICENSE`
- `QUICKSTART.md`
- `examples/hello.dt`

## Native Dependency Strategy

Releases are built against LLVM 14. The archives currently do not bundle LLVM shared libraries. End users must install the LLVM 14 runtime on their platform before running `drat`.

This is the smallest viable cross-platform release strategy that keeps artifacts immediately usable while avoiding ad hoc redistribution of platform-specific LLVM runtime pieces.

## Packaging Scripts

- `scripts/package_release.py`
- `scripts/smoke_release.py`

Use them locally to sanity-check the release flow before tagging.

Example:

```sh
cargo build -p drat -p draton-runtime
python3 scripts/package_release.py \
  --binary target/debug/drat \
  --artifact draton-linux-x86_64.tar.gz \
  --out-dir dist
python3 scripts/smoke_release.py --archive dist/draton-linux-x86_64.tar.gz
```
