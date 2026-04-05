# GitHub Release Workflow

The release pipeline is defined in `.github/workflows/release.yml`.

## Branch promotion policy

Draton uses three long-lived branches:

- `dev` for active development and frequent integration
- `unstable` for release-candidate style validation and broader testing
- `main` for code that is already considered stable

The expected promotion path is:

```text
dev -> unstable -> main
```

Release preparation should follow that flow. `main` is not the place to discover whether a change is safe; that confidence should already come from validation on `dev` and `unstable`.

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
- an optional bundled `llvm/` toolchain directory when `scripts/package_release.py` is given a local LLVM bundle path
- `LICENSE`
- `QUICKSTART.md`
- `examples/hello.dt`
- `examples/early-preview/hello-app/`
- `INSTALL.md`
- `EARLY-PREVIEW.md`
- `install.sh`
- `install.ps1`

## Native Dependency Strategy

Releases are built against vendored LLVM 18.1.8 fetched through `scripts/vendor_llvm.py`, but the shipped `drat` binaries on verified preview targets do not currently depend on an external LLVM shared library at runtime. The smoke test strips LLVM-specific environment variables from the packaged-artifact run and checks the packaged binary for accidental `libLLVM` / `clang-cpp` dynamic dependencies on Linux and macOS.

When the archive includes a bundled `llvm/` directory, the smoke test also points `DRATON_LLVM_BUNDLE_PREFIX` at that packaged toolchain and scrubs common compiler/linker environment overrides before running `drat build`.

For source-build verification, prefer `python3 scripts/vendor_llvm.py print-env --target host` over calling the raw bundle `llvm-config` directly. The script now prepares a repo-local `llvm-config` shim inside the vendored prefix so `llvm-sys` can query version and library metadata even when the upstream bundle executable is not runnable on the current Linux host.

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
python3 scripts/vendor_llvm.py fetch --target host
eval "$(python3 scripts/vendor_llvm.py print-env --target host)"
cargo build -p drat -p draton-runtime
python3 scripts/package_release.py \
  --binary target/debug/drat \
  --artifact draton-early-linux-x86_64.tar.gz \
  --out-dir dist
python3 scripts/smoke_release.py --archive dist/draton-early-linux-x86_64.tar.gz
```
