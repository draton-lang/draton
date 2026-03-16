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
3. Packages one archive per supported platform.
4. Verifies each archive with a smoke test:
   - `drat --version`
   - `drat fmt --check`
   - `drat lint`
   - `drat task`
   - `drat build`
   - `drat lsp` initialize
5. Generates per-archive `.sha256` files and a combined `SHA256SUMS.txt`.
6. Publishes the GitHub Release and uploads all supported assets plus install scripts.

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

Releases are built against LLVM 14, but the shipped `drat` binaries on verified preview targets do not currently depend on an external LLVM shared library at runtime. The smoke test strips LLVM-specific environment variables from the packaged-artifact run and checks the packaged binary for accidental `libLLVM` / `clang-cpp` dynamic dependencies on Linux and macOS.

The remaining runtime expectation is limited to normal OS libraries:

- Linux: common system libraries such as `libstdc++`, `libffi`, `libz`, and `libtinfo`
- macOS: standard system runtime libraries
- Windows x86_64: the normal Windows desktop runtime stack

This keeps the preview archives self-contained enough for end users without bundling fragile copies of platform runtime libraries.

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
