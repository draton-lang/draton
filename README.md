<div align="center">

<h1>Draton</h1>

<p>A compiled, statically-typed programming language with an ergonomic syntax and first-class tooling.</p>

[![Build](https://img.shields.io/github/actions/workflow/status/draton-lang/draton/ci.yml?branch=main&style=flat-square)](https://github.com/draton-lang/draton/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/drat?style=flat-square)](https://crates.io/crates/drat)

</div>

---

## Overview

Draton is a compiled language that targets LLVM for native performance. It combines a clean, expressive syntax with strong static typing, a rich type system, and an integrated toolchain (`drat`) that handles everything from project scaffolding to publishing.

The compiler is written entirely in Rust and designed as a Cargo workspace of focused, independently usable crates.

For the language design rationale behind the canonical syntax, see [docs/language-manifesto.md](docs/language-manifesto.md). For migration details and compatibility rules, see [docs/syntax-migration.md](docs/syntax-migration.md).

## Features

| | |
|---|---|
| **LLVM backend** | Native code generation via LLVM 14, with release-mode optimizations |
| **Static typing** | Full type inference — annotate only what you need to |
| **Classes & interfaces** | Inheritance, interface implementation, and named method layers |
| **Enums & errors** | First-class enum and structured error types |
| **Result types** | Built-in `Ok`/`Err` values and nullish-coalescing (`??`) |
| **Concurrency** | Channels (`chan[T]`) and `spawn` for lightweight concurrent tasks |
| **Pattern matching** | Exhaustive `match` expressions with structured arms |
| **Low-level escape hatches** | `unsafe`, `@pointer`, inline `asm`, and comptime blocks |
| **Integrated toolchain** | Format, lint, test, doc, REPL, LSP — all via a single `drat` binary |

## Quick Start

### Prerequisites

- LLVM 14 runtime libraries

If you are installing from a GitHub Release archive, start with [docs/install.md](docs/install.md). Current prebuilt releases expect LLVM 14 to be available on the target machine.

### Install Prebuilt Releases

Download the archive for your platform from the [GitHub Releases](https://github.com/draton-lang/draton/releases) page, extract it, add the extracted directory to `PATH`, then verify:

```sh
drat --version
drat run examples/hello.dt
```

Release artifacts:

- `draton-linux-x86_64.tar.gz`
- `draton-linux-aarch64.tar.gz`
- `draton-macos-x86_64.tar.gz`
- `draton-macos-aarch64.tar.gz`
- `draton-windows-x86_64.zip`

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

Strict mode currently targets the Rust frontend/tooling path. The self-host mirror is closer to canonical syntax than before, but it does not yet have full semantic parity for every `@type` workflow.

Canonical `@type` blocks are currently supported at file, class, layer, interface, and function scope in the Rust frontend/tooling path.

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
├── drat/               # CLI binary and compiler driver
├── draton-lexer/       # Source tokenization
├── draton-ast/         # AST node definitions
├── draton-parser/      # Token stream → AST
├── draton-typeck/      # Type checker and inference
├── draton-codegen/     # LLVM IR generation
├── draton-runtime/     # Runtime support library
└── draton-stdlib/      # Standard library
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
