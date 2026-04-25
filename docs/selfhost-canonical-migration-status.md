# Self-Host Canonical Migration Status

Current status date: April 25, 2026

This document is the canonical status sheet for the in-tree self-host compiler work.

## Authority and boundary

- The Rust workspace under `crates/` remains the authoritative implementation for compiler behavior, runtime ABI, CLI behavior, packaging, and tests.
- The in-tree self-host compiler lives under `compiler/`.
- The `compiler/` tree is real code, not a placeholder directory, but it is still subordinate to the Rust workspace until parity is proven stage by stage.
- `src/` remains reserved for the docs site and is not a compiler implementation tree.
- The current executable self-host path is the hidden Rust bootstrap command `drat selfhost-stage0`, which builds and runs the `compiler/` tree through Rust-owned tooling in `crates/drat/src/commands/selfhost_stage0.rs`.

## Current stage summary

### Phase 1 parity contract freeze

The current Phase 1 outcome is a frozen oracle surface for the stages that already exist in stage0. This is a parity-contract milestone, not a frontend-complete or backend-complete milestone.

- `drat selfhost-stage0 lex`, `parse`, `typeck`, and `build` now target one versioned envelope shape: `draton.selfhost.stage0/v1`.
- The frozen envelope fields are `schema`, `stage`, `input_path`, `bridge`, `success`, `result`, and `error`.
- The frozen stage artifacts are:
  - lexer: token stream plus lex diagnostics
  - parser: lex diagnostics plus parse diagnostics, parse warnings, and AST program payload
  - typechecker: lex diagnostics, parse diagnostics, parse warnings, type diagnostics, type warnings, and typed program payload
  - build: output artifact paths plus machine-readable build failure payload
- This does not mean parser, typechecker, ownership, or backend parity is complete. It only means the Rust-authoritative oracle surface for the current stage0 path is now explicit and machine-checkable.

### Lexer parity

- Current status: partially real in Draton; Rust is still authoritative.
- Source of truth: `crates/draton-lexer`, especially `crates/draton-lexer/tests/selfhost_parity.rs`.
- What is already real:
  - `compiler/lexer/lexer.dt`, `compiler/lexer/token.dt`, `compiler/lexer/errors.dt`, and `compiler/lexer/result.dt` contain a real lexer rewrite.
  - `compiler/driver/pipeline.dt` implements `lex_json` in Draton and does not call a `host_lex_json` bridge.
  - `compiler/main.dt` exposes the `lex` stage0 entrypoint.
  - `crates/drat/src/commands/selfhost_stage0.rs` normalizes `lex` output into the frozen `draton.selfhost.stage0/v1` envelope so CI can diff the lexer oracle without shape fallbacks.
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

- Current status: the hidden `drat selfhost-stage0 parse` command no longer dispatches through `host_parse_json`, but Rust remains the authoritative parity oracle and full parser parity is not complete yet.
- Source of truth: `crates/draton-parser`, especially `crates/draton-parser/tests/selfhost_parity.rs`.
- What is already real:
  - `compiler/parser/parser.dt` and the parser subtrees under `compiler/parser/parse/` contain an in-tree parser rewrite.
  - `compiler/ast/` contains the self-host AST model used by the rewrite.
- What is already real in stage0:
  - `crates/drat/src/commands/selfhost_stage0.rs` exposes the parser stage through the frozen `draton.selfhost.stage0/v1` envelope with `bridge.kind = "selfhost"` and `bridge.builtin = null`.
  - Hidden stage0 `parse` dispatches to `parse_json` in `compiler/driver/pipeline.dt`, which is a bridge-free Draton staging parser surface for lex diagnostics, parse diagnostics, parse warnings, and top-level item-kind payloads.
  - `crates/draton-parser/tests/selfhost_parity.rs` contains a representative-fixture parity suite that locks exact `lex_errors`, `parse_errors`, and `parse_warnings` envelopes plus top-level item-kind parity, but the suite is currently kept ignored while the bridge-free parser payload catches up to the Rust oracle.
- What still depends on Rust authority:
  - `crates/drat/src/commands/selfhost_stage0.rs` still owns stage0 bootstrap, caching, and envelope normalization.
  - `crates/draton-parser/tests/selfhost_parity.rs` remains the authoritative parser oracle.
  - The bridge-free parser surface in `compiler/driver/pipeline.dt`, and any repaired full parser serializer under `compiler/driver/parse_stage.dt`, must keep matching the Rust parser envelope and gated parser surfaces.
- Blockers:
  - Stage0 bootstrap still goes through Rust-owned build orchestration and runtime/codegen infrastructure.
  - The current full self-host parser path in `D:/draton/compiler/driver/parse_stage.dt`, `D:/draton/compiler/parser/parser.dt`, `D:/draton/compiler/parser/parse/stmts.dt`, and `D:/draton/compiler/parser/parse/types.dt` does not yet typecheck cleanly under stage0.
  - The bridge-free staging parser in `D:/draton/compiler/driver/pipeline.dt` is not yet a full Rust-shaped parser parity implementation.
  - `crates/draton-parser/tests/selfhost_parity.rs` remains the authoritative parser oracle for first-diff reporting.
  - Full Rust-shaped AST JSON parity is still not claimed; the current parser contract gates representative fixtures and top-level item kinds rather than the entire AST payload.
- Exit criteria:
  - Hidden `drat selfhost-stage0 parse` continues using no `host_parse_json` bridge.
  - Stage0 parse JSON comes from either the repaired Draton parser path under `compiler/parser/` or an equivalent bridge-free Draton parser surface that passes the Rust oracle suite.
  - Diagnostics, warnings, recovery behavior, and top-level item kinds match the Rust parser on selected parity fixtures.

### Typechecker parity

- Current status: the self-host typechecker tree under `compiler/typeck/` is growing real ownership metadata, but the hidden `drat selfhost-stage0 typeck` command still normalizes the Rust `host_type_json` bridge by default.
- Source of truth: `crates/draton-typeck`.
- What is already real:
  - `compiler/typeck/infer/`, `compiler/typeck/types/`, and `compiler/typeck/typed/` contain a real self-host typechecker tree.
  - `compiler/typeck/typed/program.dt` and related files define typed-program structures in the self-host tree.
- What is already real in the self-host tree:
  - `compiler/driver/typeck_stage.dt` contains a self-host lexer/parser/typechecker entrypoint and now serializes typed bodies, ownership summaries, and selected `use_effect` metadata.
  - `compiler/typeck/infer/result.dt` now threads the post-inference ownership pass through the self-host typed program.
- What is already real in stage0:
  - `crates/drat/tests/selfhost_stage0.rs` now compares Rust-oracle `use_effect` metadata on selected call/return sites so the target ownership metadata shape is gated in tests.
- What still depends on Rust authority:
  - `crates/drat/src/commands/selfhost_stage0.rs` still owns stage0 bootstrap, caching, and envelope normalization.
  - `crates/drat/src/commands/selfhost_stage0.rs` still dispatches hidden stage0 `typeck` through `host_type_json` by default.
  - `crates/draton-typeck/src/check.rs` and `crates/draton-typeck/src/ownership.rs` remain the authoritative semantic and ownership oracle.
  - The self-host typechecker JSON serializer under `compiler/driver/typeck_stage.dt` is still a secondary/raw path rather than the default hidden stage0 contract source.
- Blockers:
  - Hidden stage0 `typeck` still bridges through `host_type_json`, so default stage0 output is not yet direct evidence for the self-host typechecker implementation.
  - The self-host typed-program serializer still needs broader Rust-shape parity if it becomes the default hidden stage0 contract source.
  - `crates/draton-typeck/src/check.rs` and `crates/draton-typeck/src/ownership.rs` remain the authoritative semantic and ownership logic that the self-host tree still has to match.
- Exit criteria:
  - `compiler/driver/pipeline.dt` no longer calls `host_type_json`.
  - Stage0 typecheck JSON comes from the Draton typechecker path under `compiler/typeck/`.
  - Type diagnostics, warnings, and typed-program envelopes match the Rust authority on parity fixtures.

### Ownership parity

- Current status: partially real in the self-host tree; `compiler/typeck/infer/ownership.dt` now writes ownership summaries and selected expression `use_effect` metadata into the self-host typed program, but hidden stage0 `typeck` still exposes the Rust oracle by default.
- Source of truth: `crates/draton-typeck/src/ownership.rs` and `docs/runtime/inferred-ownership-spec.md`.
- What is already real:
  - `compiler/typeck/typed/ownership.dt` exists and establishes where ownership behavior belongs in the self-host tree.
  - Ownership-aware typed data structures already exist alongside the self-host typed-program model.
  - `compiler/typeck/infer/ownership.dt` now performs a self-host ownership-summary pass after HM inference and writes `ownership_summary` into the typed program for stage0 output.
  - `compiler/typeck/infer/ownership.dt` now also populates selected `use_effect` metadata on typed expressions using self-host desired-effect rules for lets, returns, calls, method calls, field/index reads, and common container literals.
  - `compiler/driver/typeck_stage.dt` now serializes typed function bodies and per-expression `use_effect` metadata on its raw self-host typecheck path.
- What still bridges to host Rust:
  - `compiler/driver/pipeline.dt` still routes `build_json` through `host_build_json`, so the production build path still relies on Rust ownership behavior.
  - `crates/draton-runtime/src/lib.rs` still reaches the Rust `drat build` path for production build fallback behavior.
- Blockers:
  - Hidden `drat selfhost-stage0 typeck` still goes through `host_type_json`, so the default stage0 command does not yet execute the self-host `use_effect` path directly.
  - The new self-host `use_effect` population currently covers a selected, high-value subset of expression forms rather than the full `crates/draton-typeck/src/ownership.rs` matrix.
  - Ownership diagnostics still come from the Rust authority; the self-host path does not yet match `crates/draton-typeck/src/ownership.rs` on borrow/move error reporting.
  - `crates/draton-runtime/src/lib.rs` remains the production-path bridge through `host_build_json`.
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
  - `crates/drat/tests/selfhost_stage0.rs` validates machine-readable `lex`, `parse`, `typeck`, and `build` envelopes, including stable build-failure payloads.
  - `.github/workflows/ci.yml` includes a workflow-dispatch path that runs stage0 commands and uploads artifacts.
  - `crates/draton-runtime/src/lib.rs` provides a fallback/rescue path that can build or reuse the Rust `drat` binary.
- What still bridges to host Rust:
  - Stage0 binary construction still goes through Rust `build::run` in `crates/drat/src/commands/selfhost_stage0.rs`.
  - Stage0 build output still uses `host_build_json`, which can recurse into the Rust `drat build` path.
  - The bootstrap path still assumes a working Rust toolchain and matching LLVM environment.
- Blockers:
  - `crates/drat/src/commands/selfhost_stage0.rs` still owns stage0 bootstrap and cache layout.
  - `crates/draton-runtime/src/lib.rs` still owns the host fallback compiler path.
  - `.github/workflows/ci.yml` keeps parser parity as an opt-in heavier remote slice; the representative fixture suite exists, but it remains ignored while the bridge-free parser payload is expanded to Rust parity.
  - `docs/benchmarks/gc-results-2026-03-17.md` records the current bootstrap workload as blocked by `LLVM ERROR: unknown special variable`.
- Exit criteria:
  - Stage0 commands expose deterministic parity envelopes for every intended frontend stage.
  - The bootstrap story distinguishes clearly between parity checking, rescue mode, and true self-rebuild.
  - A self-rebuild path exists without presenting Rust fallback as the normal compiler path.

## Host bridges currently in use

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
- Do not claim full typechecker parity merely because the `host_type_json` bridge is gone; Rust still defines the oracle.
- Do not claim ownership parity complete while self-host `use_effect` and ownership diagnostics still lag `crates/draton-typeck/src/ownership.rs`, even though summary emission is now partially real.
- Do not claim backend independence or a production-ready self-host backend while `compiler/driver/pipeline.dt` still calls `host_build_json`.
- Do not claim `drat selfhost-stage0 build` proves self-host backend completion; today it still goes through Rust fallback infrastructure.
- Do not claim Rust is optional for bootstrap or recovery.

## Next actions

Phase 0 to Phase 1 handoff should do the following, in order:

1. Keep this status file current whenever a bridge, blocker, or parity claim changes.
2. Expand deterministic parity fixtures for `drat selfhost-stage0 lex`, `parse`, `typeck`, and `build`.
3. Immediate blocker: expand the bridge-free parser payload in `D:/draton/compiler/driver/pipeline.dt` or repair the full self-host parser path in `D:/draton/compiler/driver/parse_stage.dt`, `D:/draton/compiler/parser/parser.dt`, `D:/draton/compiler/parser/parse/stmts.dt`, and `D:/draton/compiler/parser/parse/types.dt`, then re-enable parser parity in CI.
4. Expand focused typechecker parity coverage while hidden stage0 `typeck` still normalizes `host_type_json`, so the target self-host serializer and ownership envelope stay explicit before the bridge is removed.
5. Treat parser, typechecker, ownership, backend, and bootstrap as separate parity tracks instead of one generic "self-host complete" milestone.
