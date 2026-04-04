# Draton Self-Host Roadmap

This document is the implementation roadmap for moving Draton from a Rust-authoritative compiler with an in-tree self-host rewrite to a fully self-hosted compiler toolchain whose normal path is Draton code, an LLVM-first production backend, and small target-specific assembly/runtime stubs where needed.

It is a planning and execution document. It does not change the current repository authority rules by itself.

## Repository baseline on April 4, 2026

The current repository state is not "no self-host work exists". It is "self-host work exists, but the Rust workspace is still authoritative and several critical stages still go through Rust host bridges".

Observed baseline:

- `crates/` is the active source of truth for lexing, parsing, type checking, code generation, runtime ABI, CLI, packaging, and tests.
- `compiler/` already contains a substantial Draton rewrite: lexer, AST, parser, type checker, driver scaffolding, and a large codegen tree.
- `crates/drat/src/commands/selfhost_stage0.rs` builds and runs the `compiler/` tree through the hidden `drat selfhost-stage0` command.
- `compiler/driver/pipeline.dt` currently implements `lex_json` in Draton, but `parse_json`, `typeck_json`, and `build_json` still delegate to `host_parse_json`, `host_type_json`, and `host_build_json`.
- `crates/draton-runtime/src/lib.rs` provides those `host_*` builtins and can invoke the Rust `drat` binary as a fallback compiler path.
- `compiler/codegen/llvm/*.dt` is still mostly placeholder code and is not a production backend yet.
- CI already validates a small stage0 surface in `.github/workflows/ci.yml` and `crates/drat/tests/selfhost_stage0.rs`.
- The bootstrap workload is still blocked in published benchmark artifacts by `LLVM ERROR: unknown special variable`.
- `docs/selfhost-canonical-migration-status.md` is the required status control plane for keeping self-host claims aligned with the actual `compiler/` and Rust bridge state.

That baseline leads to one conclusion:

The fastest path to a real self-hosted Draton is not to jump directly to "remove Rust". The correct path is to use Rust as the authority and rescue system until the Draton compiler can prove parity, then progressively remove Rust from the default path while keeping it available as a break-glass compiler.

## End-state

The target state is:

- the default Draton compiler binary is built from Draton sources
- the default compile pipeline no longer depends on Rust host builtins during normal operation
- release artifacts no longer require the Rust toolchain to build or run ordinary Draton programs
- the normal production backend path remains LLVM-first and is driven by a self-hosted Draton compiler rather than permanent Rust host bridges
- a separate Draton-written backend, referred to here as `DraGen`, exists as a secondary backend to prove that backend work can be implemented in Draton itself
- Rust remains in-repo as a rescue compiler, parity oracle, and rebuild path when the self-host compiler is broken
- canonical syntax, the class/layer model, and inferred ownership remain identical to the Rust implementation's current semantics

More concretely, "like Zig" for Draton should mean:

- a small trusted bootstrap base
- a self-hosted compiler that can rebuild itself
- LLVM retained as the primary production backend rather than replaced for ideological reasons
- a secondary Draton backend used to prove self-host backend capability without forcing it to become the release path immediately
- a separate rescue path that is not the normal product path

It does not mean copying Zig's exact internals or rewriting everything at once.

## Non-negotiable constraints

- `crates/` remains authoritative until parity is proven stage by stage.
- `compiler/` is the only location for active self-host compiler work.
- `src/` stays reserved for the docs site.
- Canonical syntax rules remain locked: `let`, explicit `return`, brace imports, `@type`, `class`, and `layer`.
- Inferred Ownership remains the memory model for safe code.
- The roadmap must not reintroduce GC-era behavior, shadow stacks, safepoints, or alternate canonical syntax.
- Rust may stop being the default path, but it must remain buildable enough to recover the project after self-host regressions.

## Architectural target

The target architecture should be split into four layers.

### 1. Frontend authority layer

Responsibilities:

- lexing
- parsing
- canonical syntax diagnostics
- type inference and semantic checks
- inferred ownership

Rule:

The Draton frontend must first match Rust exactly before backend independence matters.

### 2. Self-host backend layer

Responsibilities:

- lowering from typed Draton plus ownership annotations into the production LLVM path without `host_build_json`
- keeping LLVM as the primary backend for release-quality code generation
- maintaining `DraGen` as a secondary Draton-written backend for proof, experimentation, and long-term backend independence work
- handling object emission, assembly emission, calling convention details, stack layout, and data layout appropriate to each backend path

Rule:

The self-host backend plan should not stop at a fake LLVM wrapper. The primary path should remain LLVM-backed, but it must be owned by the self-hosted compiler rather than by Rust bridge calls. `DraGen` may emit object files or assembly for supported targets, but it is not the production backend by default merely because it is written in Draton.

Production rule:

- LLVM remains the main backend for normal releases and normal user builds

Secondary-backend rule:

- `DraGen` exists to prove and exercise backend implementation in Draton
- `DraGen` does not become the default backend until it reaches release-quality parity and has an explicit roadmap decision behind it

The current placeholder LLVM C-API mirror under `compiler/codegen/llvm/` is acceptable as scaffolding, but it is not the end-state.

### 3. Runtime and target support layer

Responsibilities:

- process entry and exit
- syscall or libc bridge policy
- panic and allocator ABI
- scheduler/channel/runtime services
- tiny handwritten assembly for startup, trampolines, or ABI glue where required

Rule:

Keep handwritten assembly small, explicit, and target-scoped. Most logic should stay in Draton, not in assembly.

### 4. Rescue and parity layer

Responsibilities:

- Rust fallback compiler
- parity comparison tooling
- cross-check CI
- break-glass rebuild procedure

Rule:

Rust becomes a backup toolchain, not a competing product path.

## What must change first

Before the project can claim any real self-host milestone, these truth gaps must be fixed:

1. Restore `docs/selfhost-canonical-migration-status.md` and keep it current.
2. Record exactly which `compiler/` stages are real, which stages are bridged, and which stages are placeholders.
3. Separate "frontend parity complete" from "backend self-host complete" in all docs and CI labels.
4. Stop treating any successful `selfhost-stage0 build` result as evidence of backend independence while `host_build_json` remains in use.

Items 1 and 2 were completed for the current repository state on April 4, 2026. They remain ongoing maintenance requirements after that restoration pass.

## Phase 0: Re-establish truthful status tracking

Goal:

Create a fully truthful control plane for self-host work.

Required work:

- maintain `docs/selfhost-canonical-migration-status.md`
- document current ownership of `compiler/main.dt`, `compiler/driver/pipeline.dt`, `compiler/parser/**`, `compiler/typeck/**`, and `compiler/codegen/**`
- mark every `host_*` bridge explicitly
- mark every backend placeholder under `compiler/codegen/llvm/**`, `compiler/codegen/emit/**`, and related files
- define a stage table: lexer parity, parser parity, typechecker parity, ownership parity, backend parity, bootstrap parity

Exit criteria:

- no self-host doc claims a stage is "done" when it still depends on a host bridge
- every missing or placeholder stage is visible in one status document

Status on April 4, 2026:

- `docs/selfhost-canonical-migration-status.md` has been restored as the Phase 0 status sheet
- repo-level self-host docs should describe `compiler/` as a real in-tree rewrite that is still subordinate to `crates/`

## Phase 1: Freeze parity contracts between Rust and Draton

Goal:

Turn the Rust implementation into a precise oracle for the self-host rewrite.

Required work:

- define stable parity outputs for tokens, AST, parse diagnostics, type diagnostics, typed program data, ownership summaries, IR shape, and final binary behavior
- expand machine-readable parity fixtures instead of relying only on ad hoc smoke output
- keep the Rust side authoritative while shrinking ambiguity
- ensure `drat selfhost-stage0 lex|parse|typeck|build` has deterministic JSON envelopes

Key files:

- `crates/draton-lexer/tests/selfhost_parity.rs`
- `crates/draton-parser/tests/selfhost_parity.rs`
- `crates/drat/tests/selfhost_stage0.rs`
- `.github/workflows/ci.yml`

Exit criteria:

- every frontend stage has golden parity fixtures
- parity failures report the first semantic difference, not just "command failed"

## Phase 2: Finish frontend parity in Draton

Goal:

Remove Rust host bridges from lexing, parsing, and type checking.

Required work:

- keep the Draton lexer authoritative for stage0 and broaden its parity coverage
- replace `host_parse_json` with the real Draton parser path
- replace `host_type_json` with the real Draton typechecker path
- align warnings, error codes, spans, and strict-syntax behavior with Rust
- ensure deprecated syntax handling stays identical to the Rust path until the Rust path is no longer needed as authority

Key files:

- `compiler/driver/pipeline.dt`
- `compiler/parser/**`
- `compiler/typeck/**`
- `crates/draton-parser/**`
- `crates/draton-typeck/**`

Exit criteria:

- `selfhost-stage0 parse` uses no host parser bridge
- `selfhost-stage0 typeck` uses no host typechecker bridge
- parser and typechecker parity suites pass against representative repository fixtures

## Phase 3: Port inferred ownership fully into the self-host compiler

Goal:

Move beyond Hindley-Milner parity and make self-host lowering semantically real for Draton's actual memory model.

Required work:

- port ownership state tracking, move/borrow/escape logic, diagnostics, and free-point selection
- represent ownership summaries in the self-host typed program
- compare ownership diagnostics and free-point behavior against Rust
- make ownership part of the self-host parity contract instead of leaving it outside "Phase 1"

Key files:

- `compiler/typeck/typed/ownership.dt`
- `compiler/typeck/typed/**`
- `crates/draton-typeck/src/ownership.rs`
- `docs/runtime/inferred-ownership-spec.md`

Exit criteria:

- self-host typecheck plus ownership matches Rust on selected programs
- no safe-code lowering path depends on Rust-only ownership behavior

## Phase 4: Replace the fake backend surface with a real LLVM-first self-host backend and an auxiliary Draton backend

Goal:

Stop routing binary production through `host_build_json` and stop treating placeholder LLVM wrappers as meaningful backend work.

Required work:

- introduce a real backend plan under `compiler/codegen/`
- make the LLVM path the real self-host production backend instead of a Rust-bridged placeholder path
- define `DraGen` explicitly as a secondary backend rather than an accidental parallel implementation
- choose an initial supported target set
- allow `DraGen` to emit object files directly or assembly plus a minimal external assembly/link step for the targets it supports
- keep target-specific handwritten assembly small and isolated
- provide real implementations for code emission instead of placeholder return values

Recommended target order:

1. `x86_64-linux-musl` or `x86_64-linux-gnu`
2. `aarch64-linux-musl` or `aarch64-linux-gnu`
3. macOS and Windows after Linux self-bootstrap is stable

Why this order:

- current CI and release workflow already fit Linux best
- it reduces the number of calling-convention and object-format variables during bring-up

Key files:

- `compiler/codegen/core/**`
- `compiler/codegen/emit/**`
- `compiler/codegen/llvm/**`
- `compiler/codegen/mono/**`
- `compiler/codegen/vtable/**`
- `crates/draton-codegen/**`

Exit criteria:

- `selfhost-stage0 build` no longer calls `host_build_json`
- the self-hosted compiler can drive the LLVM production path without invoking Rust compiler logic during the compile path
- `DraGen` can compile at least a clearly scoped target subset well enough to prove that backend code can be authored and maintained in Draton

## Phase 5: Build a real bootstrap ladder

Goal:

Move from "Rust can run the Draton compiler tree" to "Draton can rebuild itself".

Required stages:

- Stage R: Rust compiler builds the self-host compiler from `compiler/`
- Stage D1: the resulting self-host compiler builds itself once
- Stage D2: the newly built compiler rebuilds itself again
- Compare D1 and D2 outputs for strong semantic equivalence and, where practical, reproducibility

Required work:

- formalize the bootstrap commands
- add cache and artifact naming that preserves stage identity
- record which stage produced which binary
- define acceptable output-difference rules for non-bit-reproducible artifacts

Exit criteria:

- D1 can compile the compiler source tree
- D2 can rebuild D1's source tree successfully
- bootstrap success is a required CI signal for the supported target set

## Phase 6: Turn Rust into a rescue compiler instead of the default compiler

Goal:

Make self-host the public default while preserving Rust as a recovery mechanism.

Required work:

- keep the public `drat` interface stable
- decide whether Rust remains as `drat-rust`, `drat rescue`, or a hidden recovery command
- preserve the ability to rebuild a broken self-host compiler from a known-good Rust path
- keep cross-check CI running so rescue rot does not accumulate
- make release automation able to package both the normal self-host toolchain and the rescue path when needed

Rules:

- rescue mode must be clearly marked
- rescue mode must not silently shadow failures in the normal self-host pipeline
- the repository must always have at least one documented path to recover from a self-host regression

Exit criteria:

- the default release artifact is self-hosted
- Rust is no longer part of the normal compile path
- Rust rescue build remains documented, tested, and intentionally separate

## Phase 7: Remove permanent bootstrap debt

Goal:

Finish the transition from scaffolding to sustainable maintenance.

Required work:

- delete or quarantine obsolete `host_*` bridges
- remove placeholder backend code after the real backend replaces it
- keep only the minimum rescue interfaces needed for recovery
- tighten CI so regressions fail immediately instead of lingering as "known blocked" workloads
- measure compile time, bootstrap time, and binary correctness over time

Exit criteria:

- no production path depends on placeholder backend code
- no production path depends on host parser/typechecker/build bridges
- bootstrap debt is explicit, small, and temporary

## Distribution model

The normal product should eventually ship:

- a self-hosted `drat` compiler binary
- Draton standard library and runtime code
- an LLVM-based production backend path
- small target-specific assembly or object stubs for startup and ABI glue
- no requirement to have Rust installed to compile ordinary Draton projects

The auxiliary backend story should be explicit:

- `DraGen` is shipped or documented as a secondary backend
- `DraGen` exists to prove backend implementation in Draton and to create a path toward deeper self-host independence
- `DraGen` is not implied to be the default backend unless release docs say so explicitly

The rescue story should ship separately or behind an explicit mode:

- Rust `drat` build instructions
- a rescue artifact or documented recovery workflow
- parity and bootstrap verification commands

## Verification strategy

Every phase must define both correctness gates and escape hatches.

Correctness gates:

- lexer parity
- parser parity
- typechecker parity
- ownership parity
- backend output parity on selected fixtures
- bootstrap D1 -> D2 success
- release smoke on at least one primary platform

Escape hatches:

- Rust rebuild path remains available
- parity can temporarily fall back to Rust for diagnosis, but not for calling a milestone complete

## CI policy for self-host work

CI should progress in this order:

1. fast frontend parity on every PR
2. targeted ownership parity on every PR touching ownership or lowering
3. backend smoke on supported targets
4. bootstrap ladder on scheduled runs and release candidates
5. rescue compiler verification often enough that it never silently breaks

The current "blocked bootstrap" state must become a tracked failure class with an owner and an exit condition, not a permanent background note.

## Immediate execution order

The next practical sequence for implementation should be:

1. Keep `docs/selfhost-canonical-migration-status.md` current.
2. Mark every host bridge and backend placeholder explicitly when they change.
3. Make parser parity runnable without `host_parse_json`.
4. Make typechecker parity runnable without `host_type_json`.
5. Port ownership parity fully.
6. Design the first real self-host backend target.
7. Remove `host_build_json` from the default self-host path.
8. Add D1 -> D2 bootstrap verification.
9. Flip the default product path to self-host.
10. Keep Rust as rescue and parity infrastructure.

## Anti-goals

This roadmap does not justify:

- moving compiler implementation into `src/`
- weakening canonical syntax rules for convenience
- replacing LLVM as the primary backend before there is a concrete quality and maintenance reason to do so
- keeping placeholder backend code indefinitely while calling the compiler self-hosted
- deleting the Rust compiler before the self-host path can recover itself
- broad multi-target bring-up before one target can bootstrap cleanly
- treating the rescue compiler as an invisible fallback during ordinary builds

## Definition of done

Draton can claim a real self-hosted compiler stack when all of the following are true:

- the default compiler path is implemented in Draton
- the default compiler path does not call Rust host bridges for parse, typecheck, ownership, or build
- the default production backend is LLVM-backed, real, and not placeholder code
- `DraGen` exists as a real secondary backend with clearly documented scope
- the compiler can rebuild itself through at least two consecutive stages on a supported target
- the release artifact does not require Rust for normal use
- Rust still exists as a tested rescue compiler and parity oracle

Until then, the correct language is:

Draton is on an explicit, staged path to full self-hosting, with Rust retained as the safety net until each stage has earned replacement.
