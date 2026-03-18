---
title: CLI and tooling overview
sidebar_position: 20
---

# CLI and tooling overview

Draton treats tooling as part of the language, not as an afterthought. The `drat` binary is the operational front door for the early Draton experience.

## Core commands

The current official tooling suite includes:

- `drat build`
- `drat run`
- `drat fmt`
- `drat lint`
- `drat task`
- `drat lsp`

## Build and run

Use `drat build` to compile a file or project:

```sh
drat build examples/hello.dt -o hello
```

Use `drat run` to compile and run in one step:

```sh
drat run examples/hello.dt
```

Strict syntax mode is available when you need the canonical surface enforced:

```sh
drat build --strict-syntax examples/hello.dt
```

## Format and lint

The formatter and linter are part of the first-party CLI:

```sh
drat fmt --check examples/hello.dt
drat lint examples
```

They are expected to reinforce canonical syntax and repo rules rather than invent new stylistic directions.

## Task runner

`drat task` gives Draton projects a structured task surface:

```sh
drat task
drat task build
```

The goal is to reduce ad-hoc project scripting and make the project workflow inspectable.

## Language server

`drat lsp` exposes diagnostics and editor-facing features through the existing Rust frontend:

```sh
drat lsp
```

The LSP path is built to reuse parser/typechecker knowledge rather than invent a disconnected analysis layer.

## Tooling and language philosophy

Draton’s tooling design follows the same philosophy as the language:

- one canonical syntax lane
- explicit behavior
- strong docs/runtime/tool alignment
- clear boundaries between implementation truth and compatibility support

## Tool references

- [Formatter](../tools/formatter.md)
- [Linter](../tools/linter.md)
- [Task runner](../tools/task.md)
- [Language server](../tools/lsp.md)
- [Compiler architecture](../compiler-architecture.md)
