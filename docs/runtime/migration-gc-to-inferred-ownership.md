---
title: GC to Inferred Ownership migration
sidebar_position: 32
---

# GC to Inferred Ownership migration

Draton no longer uses a tracing GC runtime for safe code. Memory management is now decided at compile time by the ownership pass described in [inferred-ownership-spec.md](inferred-ownership-spec.md), and codegen lowers safe heap allocation to `malloc` with generated last-use `free` calls.

## What changed across the six phases

1. The ownership rules were specified for value categories, moves, borrows, last-use analysis, escapes, aliasing, closures, cycles, diagnostics, and raw escape hatches.
2. Ownership state and use annotations were added to the typed AST, plus ownership-specific diagnostics and a dedicated ownership pass entrypoint.
3. `OwnershipChecker` grew into the real ownership engine: value classification, function summary inference, flow-sensitive binding analysis, closure capture handling, cycle checks, and `use_effect`/free-point annotations.
4. Codegen stopped emitting GC allocation, write barriers, safepoints, and root management for safe values, and started lowering safe ownership to `malloc` and generated `free`.
5. The GC runtime subsystem was removed from `draton-runtime`, leaving panic, scheduler, channels, builtins, and IO in place.
6. Ownership integration coverage, corpus regression guards, `@gc_config` deprecation, and migration closeout docs were added.

## Removed

- the GC runtime subsystem, roughly 2900 lines under `draton-runtime/src/gc/`
- shadow-stack and `llvm.gcroot` integration
- safepoint globals and slow paths
- GC write barriers
- GC allocation entrypoints and tests such as `draton-runtime/tests/gc_tests.rs`

## Added

- `OwnershipChecker` as the post-typecheck ownership analysis pass
- `use_effect` annotations on typed expressions for codegen consumption
- free-point insertion based on last-use analysis
- `@acyclic` support for user-asserted acyclic ownership shapes
- higher-order effect contracts such as `(String) -> borrow` and `(String) -> move`

## Before and after performance characteristics

Before:

- safe heap memory used a tracing generational collector
- codegen had to emit runtime allocation hooks, write barriers, and safepoint-related scaffolding
- latency and memory reclamation depended on collector activity

After:

- safe heap ownership is resolved at compile time
- last-use frees happen on ownership edges selected by the compiler
- safe-code allocation lowers directly to `malloc`
- there is no GC pause behavior for safe Draton values
- conservative compile-time rejection replaces runtime tracing when aliasing, escapes, or cycles cannot be proven safe

## Compatibility note

`@gc_config` is deprecated and safe to remove from existing code. The parser and typechecker still accept it for compatibility, but it has no effect because Draton now uses Inferred Ownership instead of a GC runtime.
