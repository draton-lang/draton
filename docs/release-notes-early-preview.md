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

- the LLVM 14 + `inkwell` release toolchain path is not yet verified for producing and smoke-testing a reliable `aarch64-pc-windows-msvc` `drat` binary on GitHub-hosted infrastructure

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

- LLVM 14 runtime libraries are required on the user machine
- binaries are unsigned
- macOS binaries are not notarized
- formatter comment round-tripping remains conservative in v0
- LSP completion remains basic in v0
