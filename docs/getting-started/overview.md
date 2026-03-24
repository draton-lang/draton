---
title: Getting started
sidebar_position: 2
---

# Getting started

This section is for people who want to install Draton, run real commands, and understand the current preview boundaries without reading the whole architecture first.

## Current product state

Draton is past the syntax-stabilization phase and into the tooling phase.

What is already in place:

- canonical syntax and strict mode
- Rust compiler/tooling path as the source of truth
- early tooling suite under `drat`
- GitHub release packages for supported preview targets

What is intentionally still limited:

- the release line is still called an early preview
- the docs are strict about canonical syntax and anti-drift policy
- the self-host compiler is being rewritten and is not currently shipped in-tree

## The shortest path to a working project

1. Install Draton from a release archive or installer.
2. Run `drat --version`.
3. Run `drat run examples/hello.dt`.
4. Read the [quickstart](../quickstart.md).
5. Read the [language syntax overview](../language/syntax-overview.md).

## What to expect from the language

Draton code aims to stay explicit:

```draton
let name = input("Name: ")
println(f"Hello {name}")
```

The canonical surface is built around:

- `let`
- explicit `return`
- `import { ... } from ...`
- `@type { ... }`
- `class`
- `layer`

If you come from a language with many equally blessed styles, Draton will feel narrower by design.

## Where to go next

- [Install](../install.md)
- [Quickstart](../quickstart.md)
- [Early preview](../early-preview.md)
- [Language syntax overview](../language/syntax-overview.md)
- [CLI and tooling overview](../tooling/cli-overview.md)
