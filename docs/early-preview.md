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
- LLVM 14 runtime libraries are still required on user machines
- Windows aarch64 is not published yet

## Supported platforms

Supported in this preview:

- Linux x86_64
- Linux aarch64
- macOS x86_64
- macOS aarch64
- Windows x86_64

Blocked in this preview:

- Windows aarch64

The blocker is explicit and release-engineering specific: the current LLVM 14 + `inkwell` release toolchain path is not yet verified for producing and smoke-testing a reliable `aarch64-pc-windows-msvc` `drat` binary on GitHub-hosted infrastructure.

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
