# Draton Compiler And Toolchain Architecture

This document explains how the Draton implementation is organized in the repository and how source code moves through the toolchain.

For the language-side architectural model, see [language-architecture.md](language-architecture.md). For visual summaries, see [language-class-diagram.md](language-class-diagram.md). For the compact architecture checklist, see [language-analyst-artifact.md](language-analyst-artifact.md). For current self-host migration status, see [selfhost-canonical-migration-status.md](selfhost-canonical-migration-status.md).

## Source of truth

The Rust frontend and tooling path is the authoritative implementation.

The in-tree self-host rewrite under `compiler/` is subordinate work for parity and bootstrap preparation. It does not redefine compiler behavior while `crates/` remains active.

That means:

- parser behavior is defined by the Rust crates
- canonical syntax support is defined by the Rust crates
- typechecker and code generation behavior is defined by the Rust crates

## Workspace layout

The repository is a Cargo workspace of focused crates under `crates/`:

- `crates/draton-lexer`
- `crates/draton-ast`
- `crates/draton-parser`
- `crates/draton-typeck`
- `crates/draton-codegen`
- `crates/draton-runtime`
- `crates/draton-stdlib`
- `crates/draton-lsp`
- `crates/drat`

An in-tree self-host rewrite now also exists under `compiler/`. Its directory layout mirrors the compiler pipeline, but it is intentionally not the source of truth yet.

This layout is intentional. Draton is architected as a toolchain with separable layers, not as one large opaque compiler crate.

## End-to-end pipeline

The main compiler pipeline is:

1. source file loading
2. lexing
3. parsing into AST
4. type checking and inference
5. LLVM IR generation
6. object emission
7. linking against the Draton runtime
8. executable output

In simplified form:

```text
.dt source
  -> lexer
  -> parser
  -> AST
  -> typechecker
  -> typed program
  -> LLVM IR / object
  -> linker + runtime
  -> native binary
```

## Crate responsibilities

### `draton-lexer`

Responsibility:

- tokenize Draton source into lexical units
- provide the first stable surface interpretation of source text

Architectural role:

- language syntax begins here
- formatter, parser, diagnostics, and tooling all depend on token stability

### `draton-ast`

Responsibility:

- define core syntax tree structures
- provide the shared shape used across parser, typechecker, tooling, and codegen

Architectural role:

- acts as the shared language model in Rust
- keeps syntax-facing consumers aligned on the same structures

### `draton-parser`

Responsibility:

- convert tokens into AST
- enforce grammar and syntax boundaries
- parse canonical and compatibility syntax according to current repo rules

Architectural role:

- defines what source programs are syntactically legal
- is one of the key anti-drift enforcement points

### `draton-typeck`

Responsibility:

- type inference
- contract application
- interface/class checks
- exhaustiveness and semantic validation

Architectural role:

- makes the "code vs contract" split real
- interprets `@type` blocks as authoritative contracts

### `draton-codegen`

Responsibility:

- lower checked programs to LLVM IR
- emit runtime calls, type metadata, object layouts, dispatch structures, and ABI details

Architectural role:

- bridges typed Draton semantics into executable native code
- is where language semantics meet the runtime ABI

### `draton-runtime`

Responsibility:

- scheduler and channels
- panic and low-level runtime entrypoints
- builtins, IO, and runtime ABI support used by generated programs

Architectural role:

- provides the execution substrate for generated programs
- is a separate runtime layer, not codegen glue hidden inside the compiler

### `draton-stdlib`

Responsibility:

- standard library support exposed through the runtime/tooling stack

Architectural role:

- gives the language a practical standard environment without changing the core syntax model

### `draton-lsp`

Responsibility:

- diagnostics
- hover
- definition lookup
- symbol lookup
- completion

Architectural role:

- shows that Draton is a tooling-first language
- reuses frontend knowledge instead of inventing a parallel interpretation of the language

### `drat`

Responsibility:

- unified CLI for users and contributors
- build, run, fmt, lint, task, doc, lsp, and other project workflows

Architectural role:

- one tool hub over the workspace
- exposes the language as a coherent developer toolchain rather than a loose collection of binaries

## CLI architecture

`drat` is intentionally broad:

- `drat build`
- `drat run`
- `drat fmt`
- `drat lint`
- `drat task`
- `drat doc`
- `drat lsp`

This is part of Draton's architecture, not just packaging. The language is supposed to be used through an integrated toolchain.

## Runtime architecture

The runtime is a distinct subsystem with its own responsibilities:

- panic handling
- scheduler and channels
- builtin and IO entrypoints
- runtime ABI used by generated code
- libc interop for `malloc` and `free`

The compiler and runtime are therefore separated like this:

- compiler decides ownership, inserts last-use frees, and lowers safe heap allocation to `malloc`
- runtime provides the non-memory services and ABI entrypoints that generated programs still call

## Self-host status

The previous self-host compiler mirror under `src/` was intentionally retired to clear the way for the current rewrite under `compiler/`.

Current boundary:

- `compiler/` is the new self-host rewrite location as of March 25, 2026
- `src/` is used by the Docusaurus site source (`src/pages`, `src/css`)
- the Rust crates remain the only authoritative compiler/tooling implementation until the new self-host tree reaches parity
- the self-host tree is exercised through Rust stage0 parity and bootstrap scaffolding, not as the public toolchain entrypoint
- `compiler/main.dt` and `compiler/driver/pipeline.dt` are the live stage0 entrypoints
- `compiler/driver/pipeline.dt` currently implements `lex_json`, `parse_json`, and `typeck_json` in Draton, while `build_json` still bridges through `host_build_json`
- `compiler/driver/pipeline.dt` owns the current bridge-free stage0 parse payload; `compiler/driver/parse_stage.dt` remains the planned full self-host parser payload path and must keep the frozen stage0 parse contract aligned with Rust-shaped JSON before promotion
- `compiler/driver/typeck_stage.dt` now contains a raw self-host typechecker payload path with typed-body and `use_effect` serialization, but the hidden `drat selfhost-stage0 typeck` command still defaults to the Rust `host_type_json` oracle path
- `compiler/typeck/infer/ownership.dt` now adds self-host ownership-summary inference plus selected expression `use_effect` population, but ownership diagnostics and lowering semantics still remain Rust-authoritative
- `crates/drat/src/commands/selfhost_stage0.rs` now freezes the hidden stage0 oracle output into the versioned envelope `draton.selfhost.stage0/v1`; this improves parity gating, but it does not move authority away from the Rust crates or remove the remaining host bridges

The current migration status is documented in [selfhost-canonical-migration-status.md](selfhost-canonical-migration-status.md).

## Tooling and policy architecture

Draton's architecture includes policy and anti-drift layers, not just code crates.

Key documents:

- [language-manifesto.md](language-manifesto.md)
- [language-architecture.md](language-architecture.md)
- [canonical-syntax-rules.md](canonical-syntax-rules.md)
- [contributor-language-rules.md](contributor-language-rules.md)
- [syntax-migration.md](syntax-migration.md)
- [selfhost-canonical-migration-status.md](selfhost-canonical-migration-status.md)
- [AGENTS.md](https://github.com/draton-lang/draton/blob/main/AGENTS.md)

These documents are part of the architecture because they lock:

- canonical syntax
- contributor expectations
- compatibility boundaries
- self-host migration boundaries

## How changes should flow

If a syntax-facing or semantic change is legitimate, it should flow through the stack in order:

1. policy/docs
2. parser support
3. AST and typechecker semantics
4. codegen/runtime behavior
5. tooling/docs/examples/tests
6. self-host rewrite alignment if relevant

The reverse order is risky and usually causes drift.

## Practical reading order for contributors

To understand the implementation, read in this order:

1. [README.md](https://github.com/draton-lang/draton/blob/main/README.md)
2. [language-manifesto.md](language-manifesto.md)
3. [language-architecture.md](language-architecture.md)
4. [canonical-syntax-rules.md](canonical-syntax-rules.md)
5. [crates/drat/src/main.rs](https://github.com/draton-lang/draton/blob/main/crates/drat/src/main.rs)
6. `crates/draton-lexer` -> `crates/draton-parser` -> `crates/draton-typeck` -> `crates/draton-codegen`
7. [crates/draton-runtime/src/lib.rs](https://github.com/draton-lang/draton/blob/main/crates/draton-runtime/src/lib.rs)
8. [selfhost-canonical-migration-status.md](selfhost-canonical-migration-status.md)

## Architectural invariants

Unless the actual implementation changes and all linked docs are updated together, these should remain true:

- Rust frontend/tooling is authoritative
- canonical syntax is enforced by docs, tooling, and strict mode
- the runtime remains a distinct layer under generated programs
- `drat` remains the integrated user-facing toolchain entrypoint
