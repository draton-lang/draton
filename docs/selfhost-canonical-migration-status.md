# Self-Host Canonical Migration Status

Current status date: April 4, 2026

This document is the canonical status sheet for the in-tree self-host compiler work.

## Authority and boundary

- The Rust workspace under `crates/` remains the authoritative implementation for compiler behavior, runtime ABI, CLI behavior, packaging, and tests.
- The in-tree self-host compiler lives under `compiler/`.
- The `compiler/` tree is real code, not a placeholder directory, but it is still subordinate to the Rust workspace until parity is proven stage by stage.
- `src/` remains reserved for the docs site and is not a compiler implementation tree.
- The current executable self-host path is the hidden Rust bootstrap command `drat selfhost-stage0`, which builds and runs the `compiler/` tree through Rust-owned tooling in `crates/drat/src/commands/selfhost_stage0.rs`.

## Current stage summary

### Lexer parity

- Current status: partially real in Draton; Rust is still authoritative.
- Source of truth: `crates/draton-lexer`, especially `crates/draton-lexer/tests/selfhost_parity.rs`.
- What is already real:
  - `compiler/lexer/lexer.dt`, `compiler/lexer/token.dt`, `compiler/lexer/errors.dt`, and `compiler/lexer/result.dt` contain a real lexer rewrite.
  - `compiler/driver/pipeline.dt` implements `lex_json` in Draton and does not call a `host_lex_json` bridge.
  - `compiler/main.dt` exposes the `lex` stage0 entrypoint.
- What still bridges to host Rust:
  - Bootstrap and execution still depend on `crates/drat/src/commands/selfhost_stage0.rs`, which builds the stage0 binary with the Rust toolchain.
  - The compiled stage0 binary still runs on the Rust-owned codegen/runtime stack from `crates/`.
- Blockers:
  - `crates/drat/src/commands/selfhost_stage0.rs` still owns stage0 build orchestration.
  - `.github/workflows/ci.yml` only exercises a small lex/typeck/build smoke surface, not broad lexer parity coverage.
  - `crates/draton-lexer/tests/selfhost_parity.rs` remains the Rust-authoritative parity oracle.
- Exit criteria:
  - Lexer parity fixtures cover representative repository inputs and fail on the first semantic drift.
  - Stage0 lex stays bridge-free at the pipeline layer and is exercised broadly enough to trust it as a parity surface.

### Parser parity

- Current status: rewrite tree exists, but stage0 parser output still comes from the Rust host bridge.
- Source of truth: `crates/draton-parser`, especially `crates/draton-parser/tests/selfhost_parity.rs`.
- What is already real:
  - `compiler/parser/parser.dt` and the parser subtrees under `compiler/parser/parse/` contain an in-tree parser rewrite.
  - `compiler/ast/` contains the self-host AST model used by the rewrite.
- What still bridges to host Rust:
  - `compiler/driver/pipeline.dt` implements `parse_json` by calling `host_parse_json(path)`.
  - `crates/draton-runtime/src/lib.rs` implements `host_parse_json_path` and exports `draton_host_parse_json`.
- Blockers:
  - `compiler/driver/pipeline.dt` still hard-codes the `host_parse_json` bridge.
  - `crates/draton-runtime/src/lib.rs` still serializes parser JSON from the Rust parser.
  - `crates/draton-parser/tests/selfhost_parity.rs` remains the authoritative parser oracle.
- Exit criteria:
  - `compiler/driver/pipeline.dt` no longer calls `host_parse_json`.
  - Stage0 parse JSON comes from the Draton parser path under `compiler/parser/`.
  - Diagnostics, spans, warnings, and recovery behavior match the Rust parser on selected parity fixtures.

### Typechecker parity

- Current status: rewrite tree exists, but stage0 typechecker output still comes from the Rust host bridge.
- Source of truth: `crates/draton-typeck`.
- What is already real:
  - `compiler/typeck/infer/`, `compiler/typeck/types/`, and `compiler/typeck/typed/` contain a real self-host typechecker tree.
  - `compiler/typeck/typed/program.dt` and related files define typed-program structures in the self-host tree.
- What still bridges to host Rust:
  - `compiler/driver/pipeline.dt` implements `typeck_json` by calling `host_type_json(path, strict_flag(strict_syntax))`.
  - `crates/draton-runtime/src/lib.rs` implements `host_type_json_path` and exports `draton_host_type_json`.
- Blockers:
  - `compiler/driver/pipeline.dt` still hard-codes the `host_type_json` bridge.
  - `crates/draton-runtime/src/lib.rs` still runs the Rust parser and Rust typechecker for typecheck JSON output.
  - `crates/draton-typeck/src/check.rs` and `crates/draton-typeck/src/ownership.rs` remain the authoritative semantic and ownership logic.
- Exit criteria:
  - `compiler/driver/pipeline.dt` no longer calls `host_type_json`.
  - Stage0 typecheck JSON comes from the Draton typechecker path under `compiler/typeck/`.
  - Type diagnostics, warnings, and typed-program envelopes match the Rust authority on parity fixtures.

### Ownership parity

- Current status: not yet proven; ownership logic exists in the self-host tree, but the stage0 semantic path still bridges around it through Rust.
- Source of truth: `crates/draton-typeck/src/ownership.rs` and `docs/runtime/inferred-ownership-spec.md`.
- What is already real:
  - `compiler/typeck/typed/ownership.dt` exists and establishes where ownership behavior belongs in the self-host tree.
  - Ownership-aware typed data structures already exist alongside the self-host typed-program model.
- What still bridges to host Rust:
  - `compiler/driver/pipeline.dt` routes `typeck_json` through `host_type_json`, so ownership-relevant JSON still comes from Rust.
  - `crates/draton-runtime/src/lib.rs` uses the Rust `TypeChecker` path for stage0 semantic output.
- Blockers:
  - `compiler/driver/pipeline.dt` still bypasses self-host ownership logic through `host_type_json`.
  - `crates/draton-runtime/src/lib.rs` remains the semantic bridge.
  - `crates/draton-typeck/src/ownership.rs` remains the authoritative ownership implementation that the self-host tree has not yet matched.
  - `docs/runtime/inferred-ownership-spec.md` remains ahead of any proven self-host ownership parity claim.
- Exit criteria:
  - Ownership summaries and diagnostics are emitted from the self-host typechecker path.
  - Ownership behavior matches the Rust authority on selected programs.
  - No safe-code lowering claim depends on Rust-only ownership behavior.

### Backend parity

- Current status: not at parity; the self-host backend tree exists, but the build path is still bridged to the Rust host compiler and several LLVM-layer files are placeholder stubs.
- Source of truth: `crates/draton-codegen` and the Rust runtime/link flow used by `drat build`.
- What is already real:
  - `compiler/codegen/` contains a broad rewrite tree for codegen structure, monomorphization, vtables, layout, and emission scaffolding.
  - `compiler/codegen/core/`, `compiler/codegen/emit/`, `compiler/codegen/mono/`, `compiler/codegen/typemap/`, and `compiler/codegen/vtable/` are populated with real in-tree Draton files.
- What still bridges to host Rust:
  - `compiler/driver/pipeline.dt` implements `build_json` by calling `host_build_json(path, output, mode, strict_flag(strict_syntax), target)`.
  - `crates/draton-runtime/src/lib.rs` implements `host_build_json_path`, `runtime_ensure_host_drat`, and `host_build_source_impl`.
  - `crates/draton-runtime/src/lib.rs` can build or reuse the Rust `drat` binary and then invokes `drat build` as the fallback compiler path.
- Blockers:
  - `compiler/driver/pipeline.dt` still hard-codes the `host_build_json` bridge.
  - `crates/draton-runtime/src/lib.rs` still shells out to the Rust `drat` build path from `host_build_source_impl`.
  - `compiler/codegen/llvm/builder.dt`, `compiler/codegen/llvm/context.dt`, `compiler/codegen/llvm/module.dt`, `compiler/codegen/llvm/pass.dt`, `compiler/codegen/llvm/target.dt`, `compiler/codegen/llvm/types.dt`, and `compiler/codegen/llvm/values.dt` still expose placeholder or stub behavior.
- Exit criteria:
  - `compiler/driver/pipeline.dt` no longer calls `host_build_json`.
  - The default self-host build path emits real backend output from `compiler/codegen/`.
  - The backend no longer depends on placeholder LLVM wrapper behavior for normal compilation.

### Bootstrap parity

- Current status: bootstrap and rescue layers exist and are exercised, but they are still Rust-owned fallback infrastructure rather than self-host independence.
- Source of truth: `crates/drat/src/commands/selfhost_stage0.rs`, `crates/drat/tests/selfhost_stage0.rs`, and `.github/workflows/ci.yml`.
- What is already real:
  - `crates/drat/src/commands/selfhost_stage0.rs` builds and runs the `compiler/` tree through the hidden `drat selfhost-stage0` command.
  - `crates/drat/tests/selfhost_stage0.rs` validates machine-readable `lex`, `typeck`, and `build` envelopes.
  - `.github/workflows/ci.yml` includes a workflow-dispatch path that runs stage0 commands and uploads artifacts.
  - `crates/draton-runtime/src/lib.rs` provides a fallback/rescue path that can build or reuse the Rust `drat` binary.
- What still bridges to host Rust:
  - Stage0 binary construction still goes through Rust `build::run` in `crates/drat/src/commands/selfhost_stage0.rs`.
  - Stage0 build output still uses `host_build_json`, which can recurse into the Rust `drat build` path.
  - The bootstrap path still assumes a working Rust toolchain and matching LLVM environment.
- Blockers:
  - `crates/drat/src/commands/selfhost_stage0.rs` still owns stage0 bootstrap and cache layout.
  - `crates/draton-runtime/src/lib.rs` still owns the host fallback compiler path.
  - `.github/workflows/ci.yml` only proves a narrow stage0 smoke surface.
  - `docs/benchmarks/gc-results-2026-03-17.md` records the current bootstrap workload as blocked by `LLVM ERROR: unknown special variable`.
- Exit criteria:
  - Stage0 commands expose deterministic parity envelopes for every intended frontend stage.
  - The bootstrap story distinguishes clearly between parity checking, rescue mode, and true self-rebuild.
  - A self-rebuild path exists without presenting Rust fallback as the normal compiler path.

## Host bridges currently in use

- `host_parse_json`
  - Called from `compiler/driver/pipeline.dt`.
  - Implemented by `crates/draton-runtime/src/lib.rs` through `host_parse_json_path` and `draton_host_parse_json`.
- `host_type_json`
  - Called from `compiler/driver/pipeline.dt`.
  - Implemented by `crates/draton-runtime/src/lib.rs` through `host_type_json_path` and `draton_host_type_json`.
- `host_build_json`
  - Called from `compiler/driver/pipeline.dt`.
  - Implemented by `crates/draton-runtime/src/lib.rs` through `host_build_json_path` and `draton_host_build_json`.
  - The build fallback ultimately shells out to the Rust CLI path in `crates/draton-runtime/src/lib.rs` through `runtime_ensure_host_drat` and `host_build_source_impl`.

## Known placeholder areas

These paths exist in-tree but must not be described as production-ready backend implementation yet:

- `compiler/codegen/llvm/builder.dt`
- `compiler/codegen/llvm/context.dt`
- `compiler/codegen/llvm/module.dt`
- `compiler/codegen/llvm/pass.dt`
- `compiler/codegen/llvm/target.dt`
- `compiler/codegen/llvm/types.dt`
- `compiler/codegen/llvm/values.dt`

## What must not be claimed yet

- Do not claim the self-host compiler is the authoritative implementation.
- Do not claim parser parity while `compiler/driver/pipeline.dt` still calls `host_parse_json`.
- Do not claim typechecker parity while `compiler/driver/pipeline.dt` still calls `host_type_json`.
- Do not claim backend independence or a production-ready self-host backend while `compiler/driver/pipeline.dt` still calls `host_build_json`.
- Do not claim `drat selfhost-stage0 build` proves self-host backend completion; today it still goes through Rust fallback infrastructure.
- Do not claim Rust is optional for bootstrap or recovery.

## Next actions

Phase 0 to Phase 1 handoff should do the following, in order:

1. Keep this status file current whenever a bridge, blocker, or parity claim changes.
2. Expand deterministic parity fixtures for `drat selfhost-stage0 lex`, `parse`, `typeck`, and `build`.
3. Use the Rust crates as the oracle while removing ambiguity from JSON envelopes and diagnostics.
4. Treat parser, typechecker, ownership, backend, and bootstrap as separate parity tracks instead of one generic “self-host complete” milestone.
