# Draton Early Tooling Preview

This is the first public Draton Early Tooling Preview release.

## Included

- `drat` compiler CLI
- `drat fmt`
- `drat lint`
- `drat task`
- `drat lsp`
- canonical example files and a bundled sample project

## Supported platforms

- Linux x86_64
- Linux aarch64
- macOS x86_64
- macOS aarch64
- Windows x86_64

## Blocked platform

- Windows aarch64

Current blocker:

- LLVM 14 does not currently have a published `win32/arm64` prebuilt asset in the release toolchain matrix Draton uses for `inkwell` / `llvm-sys 14`, so there is no verified way to build and smoke-test a release-quality `aarch64-pc-windows-msvc` `drat` binary without maintaining a separate LLVM 14 arm64 Windows toolchain

## Install

See:

- `docs/install.md`
- `docs/quickstart.md`
- `docs/early-preview.md`

Quick verification:

```sh
drat --version
drat fmt --check examples/early-preview/hello-app/src
drat lint examples/early-preview/hello-app/src
cd examples/early-preview/hello-app
drat task
drat task build
```

## Known limitations

- Linux preview binaries still depend on standard system runtime libraries such as `libstdc++`, `libffi`, `libz`, and `libtinfo`
- binaries are unsigned
- macOS binaries are not notarized
- formatter comment round-tripping remains conservative in v0
- LSP completion remains basic in v0
