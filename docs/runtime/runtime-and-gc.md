---
title: Runtime and Ownership
sidebar_position: 30
---

# Runtime and Ownership

Draton’s runtime is the low-level layer that makes compiled programs executable. With Inferred Ownership, memory management for safe Draton values is now decided at compile time instead of by a tracing collector in the runtime.

## Runtime role

The compiler pipeline ends in native code linked against the Draton runtime. That means runtime behavior is part of the real language experience, not just an implementation detail hidden from users.

Key responsibilities include:

- runtime string and array helpers
- output/input builtins
- panic/runtime support
- scheduling and concurrency primitives where needed

## Ownership model

Safe Draton programs now use Inferred Ownership:

- the typechecker infers copy, borrow, and move at use sites
- codegen emits `malloc` and `free` directly for owned heap values
- the runtime no longer performs tracing, barriers, or safepoint-driven reclamation for safe values
- explicit escape hatches such as `@pointer` remain outside the inferred ownership model

This keeps the runtime smaller and shifts correctness checks to compile time, where aliasing and ownership ambiguity are rejected before code is emitted.

## Runtime boundaries

The runtime still owns:

- the ABI surface used by generated code
- string, file, and CLI helpers
- panic entrypoints
- the scheduler, channels, and coroutine support
- host-tooling helpers used by `drat` and bootstrap flows

The runtime no longer owns:

- tracing GC state
- write barriers
- safepoint polling
- shadow-stack metadata
- type-descriptor registration for a collector

## Archived GC material

Older GC benchmarking material is retained only as historical context:

- [GC scorecard](../gc-scorecard.md)
- benchmark artifacts under `docs/benchmarks/`

Those documents are archived and should not be treated as describing the active runtime architecture.
