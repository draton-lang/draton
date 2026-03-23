---
title: Draton docs overview
slug: /
sidebar_position: 1
---

# Draton docs overview

This site is the authoritative manual for the Draton language and its toolchain.

If you are new to Draton, use this order:

1. [Getting started overview](getting-started/overview.md)
2. [Install](install.md)
3. [Quickstart](quickstart.md)
4. [Language syntax overview](language/syntax-overview.md)
5. [CLI and tooling overview](tooling/cli-overview.md)

If you are contributing to the language or compiler, use this order:

1. [Language manifesto](language-manifesto.md)
2. [Canonical syntax rules](canonical-syntax-rules.md)
3. [Contributor language rules](contributor-language-rules.md)
4. [Language architecture](language-architecture.md)
5. [Compiler architecture](compiler-architecture.md)

## What Draton is

Draton is a compiled, statically typed, tooling-first language. Its design is intentionally narrow in the places that most languages let drift:

- readability comes first
- code expresses behavior
- `@type` expresses contracts
- canonical syntax has one preferred shape
- `class` defines structure
- `layer` groups capabilities
- the toolchain is part of the language experience, not an optional extra

Those rules are not cosmetic. They are the boundaries that keep the language, the compiler, the self-host mirror, and the docs aligned.

## Documentation map

### Getting started

Use the getting-started section for installation, early preview boundaries, and a runnable first project.

### Language

Use the language section to understand the syntax surface, contract model, control flow, structural model, and system builtins.

### Tooling

Use the tooling section to understand `drat build`, `drat run`, `drat fmt`, `drat lint`, `drat task`, and `drat lsp`.

### Compiler and runtime

Use the compiler and runtime section to understand the Rust frontend, the self-host mirror, the Inferred Ownership memory model, and the remaining runtime services. Archived GC benchmark material is retained only for historical context.

### Contributor rules

Use the contributor section when making syntax, tooling, docs, self-host, release, or policy changes.

## Current readiness

Draton has stabilized its canonical syntax and ships an early tooling preview with:

- compiler and CLI
- formatter
- linter
- task runner
- language server
- strict syntax enforcement
- focused strict self-host CI coverage

The executable self-host path is canonicalized. Only deferred dump/printer cleanup remains outside full-tree strict self-host coverage.
