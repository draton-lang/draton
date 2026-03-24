<div align="center">

<h1>Draton</h1>

<p>A compiled, statically-typed programming language with an ergonomic syntax and first-class tooling.</p>

[![Build](https://img.shields.io/github/actions/workflow/status/draton-lang/draton/ci.yml?branch=main&style=flat-square)](https://github.com/draton-lang/draton/actions)
[![Docs](https://img.shields.io/github/actions/workflow/status/draton-lang/draton/docs.yml?branch=main&label=docs&style=flat-square)](https://github.com/draton-lang/draton/actions/workflows/docs.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/drat?style=flat-square)](https://crates.io/crates/drat)

</div>

---
> [!CAUTION]
> **[v0.1.42](https://github.com/draton-lang/draton/releases/tag/v0.1.42) is still an early preview, but it fixes the Windows x86_64 crash in `println(Int)` / range-loop demos and the default `.exe` handling for single-file `drat build` and `drat run`.**

## Overview

Draton is a compiled language that targets LLVM for native performance. It combines a clean, expressive syntax with strong static typing, a rich type system, and an integrated toolchain (`drat`) that handles everything from project scaffolding to publishing.

The compiler is written entirely in Rust and designed as a Cargo workspace of focused, independently usable crates.

For the full documentation set, use the Docusaurus docs site:

- <https://docs.draton.lhqm.io.vn>

It is built from the `docs/` tree and deployed by GitHub Actions. The repo-local source material starts here:

- [docs/intro.md](docs/intro.md)
- [docs/language-manifesto.md](docs/language-manifesto.md)
- [docs/language-architecture.md](docs/language-architecture.md)
- [docs/language-class-diagram.md](docs/language-class-diagram.md)
- [docs/language-analyst-artifact.md](docs/language-analyst-artifact.md)
- [docs/compiler-architecture.md](docs/compiler-architecture.md)
- [docs/canonical-syntax-rules.md](docs/canonical-syntax-rules.md)
- [docs/contributor-language-rules.md](docs/contributor-language-rules.md)
- [docs/syntax-migration.md](docs/syntax-migration.md)
- [docs/roadmap-1year.md](docs/roadmap-1year.md)

Runtime and memory-model references:

- [docs/runtime/runtime-and-gc.md](docs/runtime/runtime-and-gc.md)
- [docs/runtime/migration-gc-to-inferred-ownership.md](docs/runtime/migration-gc-to-inferred-ownership.md)
- [docs/runtime/inferred-ownership-spec.md](docs/runtime/inferred-ownership-spec.md)

Tooling references:

- [docs/tools/formatter.md](docs/tools/formatter.md)
- [docs/tools/linter.md](docs/tools/linter.md)
- [docs/tools/task.md](docs/tools/task.md)
- [docs/tools/lsp.md](docs/tools/lsp.md)

## Documentation Site

The repository now includes a Docusaurus docs site for Draton’s public manual and contributor-facing architecture docs.

Local docs workflow:

```sh
npm install
npm run start
```

Static build:

```sh
npm run build
```

The docs deployment workflow publishes the built site to GitHub Pages for `docs.draton.lhqm.io.vn`. See [docs/contributor/docs-site-deployment.md](docs/contributor/docs-site-deployment.md).

## Features

| | |
|---|---|
| **LLVM backend** | Native code generation via LLVM 14, with release-mode optimizations |
| **Static typing** | Full type inference — annotate only what you need to |
| **Inferred Ownership** | Compile-time copy/borrow/move inference with generated last-use `free` for safe heap values |
| **Classes & interfaces** | Inheritance, interface implementation, and named method layers |
| **Enums & errors** | First-class enum and structured error types |
| **Result types** | Built-in `Ok`/`Err` values and nullish-coalescing (`??`) |
| **Concurrency** | Channels (`chan[T]`) and `spawn` for lightweight concurrent tasks |
| **Pattern matching** | Exhaustive `match` expressions with structured arms |
| **Low-level escape hatches** | `unsafe`, `@pointer`, inline `asm`, and comptime blocks |
| **Integrated toolchain** | Format, lint, test, doc, REPL, LSP — all via a single `drat` binary |

## Quick Start

### Prerequisites

If you are installing from a GitHub Release archive, start with [docs/install.md](docs/install.md). The Early Tooling Preview packages the compiler and codegen stack directly; users do not need to install LLVM separately on supported preview targets.

Source builds still require:

- Rust stable
- LLVM 14 development libraries

### Install Prebuilt Releases

Download the Early Tooling Preview archive for your platform from the [GitHub Releases](https://github.com/draton-lang/draton/releases) page, extract it, add the extracted directory to `PATH`, then verify:

```sh
drat --version
drat run examples/hello.dt
```

The release installers in [docs/install.md](docs/install.md) update `PATH` automatically when practical. If `drat` is not visible immediately after installation, start a new shell session.

Windows note:

- use `v0.1.42` or later if you rely on `println` with integer values, `for ... in range(...)`, or default-output single-file `drat build` / `drat run`

Release artifacts:

- `draton-early-linux-x86_64.tar.gz`
- `draton-early-linux-aarch64.tar.gz`
- `draton-early-macos-x86_64.tar.gz`
- `draton-early-macos-aarch64.tar.gz`
- `draton-early-windows-x86_64.zip`

### Install from source

Source builds require:

- Rust stable
- LLVM 14 development libraries

```sh
git clone https://github.com/draton-lang/draton.git
cd draton
cargo build --release
export PATH="$PWD/target/release:$PATH"
```

### Verify the install

```sh
drat --version
drat run examples/hello.dt
```

Expected output:

```text
hello, draton!
```

For full platform-specific install snippets, checksum verification, and troubleshooting, see [docs/install.md](docs/install.md).

## Early Tooling Experience

Draton now ships an official early tooling suite under `drat`:

```sh
drat fmt --check examples/hello.dt
drat lint examples tests
drat task
drat task build
drat lsp
```

Tooling v0 is intentionally practical rather than speculative:

- `drat fmt` provides deterministic formatting and a safe `--check` mode
- `drat lint` surfaces deprecated syntax, unused imports, unreachable code, and obvious contract issues
- `drat task` runs repository or project automation from `drat.tasks`
- `drat lsp` provides diagnostics, hover, definition, symbols, and basic completion

The repository ships a root [drat.tasks](drat.tasks) so early adopters can see the intended automation shape immediately.

## Early Preview

The first public Early Tooling Preview is aimed at real end-user installation and feedback, not just repository contributors.

Get started here:

- [docs/early-preview.md](docs/early-preview.md)
- [docs/install.md](docs/install.md)
- [docs/quickstart.md](docs/quickstart.md)
- [GitHub Releases](https://github.com/draton-lang/draton/releases)

Current published preview targets:

- Linux x86_64
- Linux aarch64
- macOS x86_64
- macOS aarch64
- Windows x86_64

Windows aarch64 is not part of the current Early Tooling Preview target set.

That blocker is tracked explicitly in [docs/early-preview.md](docs/early-preview.md). The current issue is not a vague CI gap: LLVM 14 does not have a published Windows arm64 prebuilt asset in the release toolchain matrix Draton depends on, so Draton does not claim support for that target until a verified LLVM 14 arm64 release path exists.

## Language Tour

### Variables and types

```draton
let x = 42
let mut count = 0
let name = "Draton"
let greeting = f"Hello, {name}!"

@type {
    count: Int
    name: String
}
```

### Functions

```draton
@type {
    add: (Int, Int) -> Int
}

pub fn add(a, b) {
    return a + b
}
```

### Classes and interfaces

```draton
interface Shape {
    @type {
        area: () -> Float
    }

    fn area()
}

class Circle implements Shape {
    let radius

    layer metrics {
        fn area() {
            return 3.14159 * radius * radius
        }
    }

    layer display {
        fn to_string() {
            return f"Circle(r={radius})"
        }
    }

    @type {
        radius: Float
        area: () -> Float
        to_string: () -> String
    }
}

class ColoredCircle extends Circle {
    let color

    @type {
        color: String
    }
}
```

### Imports

```draton
import { User } from models.user
import { connect, listen } from std.net
import { http as nethttp } from std.net
```

### Compatibility vs strict syntax

By default, the Rust frontend still accepts legacy inline type syntax for compatibility and emits deprecation warnings that point to the canonical `@type` form.

Use strict mode to reject deprecated syntax:

```sh
drat build --strict-syntax examples/hello.dt
drat run --strict-syntax examples/hello.dt
```

Strict mode currently targets the Rust frontend/tooling path.
Canonical `@type` blocks are currently supported at file, class, layer, interface, and function scope in the Rust frontend/tooling path.

The historical self-host compiler mirror was intentionally removed from `src/` while a rewrite is prepared. Current syntax and tooling guarantees are therefore enforced through the Rust crates and their tests/CI. The reset status is tracked in [docs/selfhost-canonical-migration-status.md](docs/selfhost-canonical-migration-status.md).

### Enums and pattern matching

```draton
enum Direction {
    North
    South
    East
    West
}

@type {
    label_for: (Direction) -> String
}

fn label_for(dir) {
    match dir {
        Direction.North => {
            return "up"
        }
        Direction.South => {
            return "down"
        }
        _ => {
            return "sideways"
        }
    }
}
```

### Error types and results

```draton
error ParseError { message: String }

@type {
    parse: (String) -> Result[Int, ParseError]
}

fn parse(input) {
    if input == "42" {
        return Ok(42)
    }

    return Err(ParseError { message: "invalid input" })
}

fn load_value() {
    return parse("abc") ?? 0
}
```

### Concurrency

```draton
let ch = chan[Int]

spawn {
    ch.send(42)
}

@type {
    read_value: () -> Int
}

fn read_value() {
    let result = ch.recv()
    return result
}
```

### Low-level blocks

```draton
@type {
    read_pointer: () -> Int
}

fn read_pointer() {
    let raw = @pointer {
        // pointer arithmetic here
    }

    let value = @comptime {
        // evaluated at compile time
    }

    return value
}
```

## CLI Reference

`drat` is the official build tool and package manager.

| Command | Description |
|---|---|
| `drat init [name]` | Scaffold a new project |
| `drat build` | Compile in debug mode |
| `drat build --strict-syntax` | Reject deprecated inline type syntax |
| `drat build --release` | Compile with optimizations |
| `drat run` | Build and run |
| `drat run --strict-syntax` | Build and run with canonical syntax enforcement |
| `drat test` | Run tests |
| `drat fmt` | Format source files |
| `drat lint` | Lint source files |
| `drat doc` | Generate documentation |
| `drat repl` | Start an interactive REPL |
| `drat lsp` | Start the Language Server |
| `drat add <pkg>` | Add a dependency |
| `drat remove <pkg>` | Remove a dependency |
| `drat update [pkg]` | Update dependencies |
| `drat publish` | Publish to the package registry |

## Repository Structure

This repository is a Cargo workspace. Each crate has a single, well-defined responsibility.

```
draton/
├── crates/
│   ├── drat/               # CLI binary and compiler driver
│   ├── draton-lexer/       # Source tokenization
│   ├── draton-ast/         # AST node definitions
│   ├── draton-parser/      # Token stream -> AST
│   ├── draton-typeck/      # Type checker and inference
│   ├── draton-codegen/     # LLVM IR generation
│   ├── draton-runtime/     # Runtime support library
│   ├── draton-stdlib/      # Standard library
│   └── draton-lsp/         # Language server
├── docs/                   # Documentation content
└── src/                    # Docusaurus site source
```

## Maintainers

This project is currently developed and maintained by:

* **Lê Hùng Quang Minh** (@lehungquangminh) — Lead Architect (Core Compiler, LLVM Backend)

## Contributing

Contributions are welcome — bug reports, feature requests, documentation improvements, and code are all appreciated. Please read [CONTRIBUTING](.github/CONTRIBUTING.md) before opening a pull request, and follow our [Code of Conduct](.github/CODE_OF_CONDUCT.md).

## Security

To report a vulnerability, please follow the process described in [SECURITY](.github/SECURITY.md). Do not open public issues for security problems.

## License

Draton is licensed under the [Apache License, Version 2.0](LICENSE).
