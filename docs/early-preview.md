# Draton Early Tooling Preview

This release is the first public Draton developer tooling preview.

It is meant for early adopters who want a real installable package, not just repository internals.

## What is included

- Draton compiler and CLI
- `drat fmt`
- `drat lint`
- `drat task`
- `drat lsp`
- canonical-syntax examples and a small sample project

## What this preview is for

- trying the Draton CLI without building from source
- testing the canonical syntax workflow
- formatting, linting, building, and editing small Draton projects
- giving feedback on early tooling experience

## What is intentionally limited

- formatter comment round-tripping is still conservative, so some comment-bearing files are skipped instead of rewritten
- LSP completion is basic and editor support is still an MVP
- binaries are unsigned
- macOS binaries are not notarized
- Linux builds still depend on a small set of standard system runtime libraries such as `libstdc++`, `libffi`, `libz`, and `libtinfo`

## Supported platforms

Supported in this preview:

- Linux x86_64
- Linux aarch64
- macOS x86_64
- macOS aarch64
- Windows x86_64

Windows x86_64 users should use `v0.1.42` or later. That preview fixes the packaged runtime/codegen mismatch that could crash integer `println(...)` and `for ... in range(...)`, and it restores the expected default `.exe` path for single-file `drat build` / `drat run`.

Windows aarch64 is not part of this preview target set.

## Install paths

Choose one:

- scripted install with `install.sh`
- scripted install with `install.ps1`
- manual archive download and extraction

Full details: [install.md](install.md)

## Verification steps

After install:

```sh
drat --version
drat fmt --check examples/early-preview/hello-app/src
drat lint examples/early-preview/hello-app/src
cd examples/early-preview/hello-app
drat task
drat task build
drat lsp
```

## Early feedback boundary

This preview is aimed at:

- tool usability
- installation friction
- CLI ergonomics
- formatter/linter output quality
- basic editor support

It is not reopening the language surface or canonical syntax rules.
