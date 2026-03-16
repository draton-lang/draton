# GitHub Release Workflow

The release pipeline is defined in `.github/workflows/release.yml`.

## Trigger

The workflow runs on:

- tags matching `v0.*`
- tags matching `early-*`

## What it does

1. Runs focused Early Preview verification on Linux.
2. Builds `drat`, `draton-runtime`, and `draton-lsp` in release mode for the supported preview targets:
   - Linux x86_64
   - Linux aarch64
   - macOS x86_64
   - macOS aarch64
   - Windows x86_64
3. Records the explicit blocker for:
   - Windows aarch64
4. Packages one archive per supported platform.
4. Verifies each archive with a smoke test:
   - `drat --version`
   - `drat fmt --check`
   - `drat lint`
   - `drat task`
   - `drat build`
   - `drat lsp` initialize
5. Generates per-archive `.sha256` files and a combined `SHA256SUMS.txt`.
6. Publishes the GitHub Release and uploads all supported assets plus install scripts and the platform blocker note.

## Archive Layout

Each shipped Early Preview archive contains:

- `drat` or `drat.exe`
- the packaged Draton runtime static library
- `LICENSE`
- `QUICKSTART.md`
- `examples/hello.dt`
- `examples/early-preview/hello-app/`
- `INSTALL.md`
- `EARLY-PREVIEW.md`
- `install.sh`
- `install.ps1`

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
  --artifact draton-early-linux-x86_64.tar.gz \
  --out-dir dist
python3 scripts/smoke_release.py --archive dist/draton-early-linux-x86_64.tar.gz
```
