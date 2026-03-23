# Draton Analyst Artifact

This document is a compact analyst-facing artifact for reviewers, contributors, and tool builders who need the architectural truth quickly.

It is intentionally denser and more checklist-oriented than [language-architecture.md](language-architecture.md).

## Executive Summary

Draton is a readability-first, statically typed, tooling-first compiled language with a canonical syntax surface and an explicit contract layer.

Its architecture is built around:

- one canonical executable surface
- one canonical contract mechanism
- one structural model based on `class` and `layer`
- one authoritative implementation path in Rust

## Architectural Identity

| Area | Draton position |
| --- | --- |
| Primary goal | Readable code with strong tooling |
| Execution model | Compiled to native code through LLVM |
| Typing model | Static typing with inference by default |
| Contract model | Explicit `@type` blocks |
| Structural model | `class` for structure, `layer` for capability grouping |
| Control flow | Explicit `return` remains canonical |
| Import model | `import { item } from module.path` |
| Tooling stance | Integrated `drat` toolchain is first-class |
| Compatibility stance | Migration support only, not co-equal design |

## Canonical Language Invariants

These invariants define the language architecture:

1. Code expresses behavior.
2. `@type` expresses contracts.
3. `let` is the canonical binding form.
4. `return` remains explicit.
5. Brace imports remain canonical.
6. `class` models structure.
7. `layer` models grouped capability.
8. Compatibility syntax does not define a second language philosophy.

## What The Language Optimizes For

Draton is optimized for:

- codebases that need strong tooling
- compiler and language-engineering work
- explicit, maintainable source structure
- projects that want inference by default and contracts when needed

It is not optimized for:

- multiple equally valid syntax dialects
- maximal terseness
- inline type density as a default experience
- reopening settled canonical syntax questions

## Separation Of Concerns

| Concern | Architectural home |
| --- | --- |
| Runtime behavior | executable code |
| Type intent | `@type` blocks |
| Structural organization | `class`, `layer`, file/module layout |
| Native execution | LLVM + runtime |
| User workflow | `drat` CLI |
| Editor support | `draton-lsp` |

## Syntax Architecture Snapshot

| Topic | Canonical | Not canonical |
| --- | --- | --- |
| Bindings | `let x = 1` | `let x: Int = 1` |
| Functions | `fn f(a) { return a }` | `fn f(a: Int) -> Int { ... }` |
| Contracts | `@type { f: (Int) -> Int }` | inline type noise as default |
| Imports | `import { x } from pkg.mod` | alternate primary import dialects |
| Control flow | explicit `return` | implicit-return-only philosophy |

## Semantic Layering

The language can be understood in three layers:

### Layer 1: executable surface

- expressions
- statements
- function bodies
- class/layer member behavior

### Layer 2: contract surface

- file/class/layer/interface/function `@type`
- interface member contracts
- explicit hints where inference needs help

### Layer 3: implementation substrate

- Rust frontend
- LLVM backend
- Draton runtime
- self-host mirror for parity and long-term self-hosting

## Implementation Truth Table

| Concern | Source of truth |
| --- | --- |
| Canonical syntax | Rust parser + docs |
| Type contract semantics | Rust typechecker + docs |
| Native codegen semantics | Rust codegen + runtime |
| Self-host behavior | Must preserve Rust parity |
| Policy and anti-drift | AGENTS + architecture/rules docs |

## Current Repository State

As of the current repository state:

- canonical syntax is stabilized
- strict syntax mode exists
- executable/compiler-path self-host migration is effectively complete
- only two deferred non-executable dump/printer files remain outside full-tree strict self-host coverage:
  - `src/ast/dump.dt`
  - `src/typeck/dump.dt`
- the self-host bootstrap path is still tracked separately because `drat build src/main.dt` can hit `LLVM ERROR: unknown special variable`

## Implications For Reviewers And Contributors

When evaluating a change, ask:

1. Does it preserve readability-first source?
2. Does it keep contracts in `@type` rather than pushing types back inline?
3. Does it preserve the `class` / `layer` model?
4. Does it keep docs, parser behavior, and examples aligned?
5. Does it maintain Rust frontend authority and self-host parity?

If the answer to any of those is "no", the default position should be to reject or narrow the change.

## Quick Component Map

| Component | Role |
| --- | --- |
| `draton-lexer` | tokenization |
| `draton-ast` | shared syntax tree model |
| `draton-parser` | grammar and AST construction |
| `draton-typeck` | inference and contract checking |
| `draton-codegen` | LLVM lowering |
| `draton-runtime` | ownership-era runtime ABI, scheduler, channels, panic, builtins, IO |
| `draton-stdlib` | host-side standard library |
| `draton-lsp` | editor protocol support |
| `drat` | unified CLI and tool hub |
| `src/` | self-host mirror of compiler layers |

## Recommended Reading Order

For architecture review:

1. [language-manifesto.md](language-manifesto.md)
2. [language-architecture.md](language-architecture.md)
3. [language-class-diagram.md](language-class-diagram.md)
4. [compiler-architecture.md](compiler-architecture.md)
5. [canonical-syntax-rules.md](canonical-syntax-rules.md)
6. [contributor-language-rules.md](contributor-language-rules.md)
7. [selfhost-canonical-migration-status.md](selfhost-canonical-migration-status.md)
