# Draton Early Tooling Preview

This is the first public Draton Early Tooling Preview release.

The current tagged preview also includes a Windows x86_64 packaging fix:

- align generated object files with the packaged MinGW target triple so `println(Int)` and `for ... in range(...)` no longer crash in shipped builds
- restore the default `.exe` output path for single-file `drat build` / `drat run`

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

## Not included in this preview

- Windows aarch64

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
