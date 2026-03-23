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
- the runtime no longer performs collector-driven memory management for safe values
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

- collector state
- barrier machinery
- polling hooks used by the old collector
- shadow-stack metadata
- type-descriptor registration for an old collector path

For the active model, pair this document with the [Inferred Ownership specification](inferred-ownership-spec.md). For historical migration context, use [migration-gc-to-inferred-ownership.md](migration-gc-to-inferred-ownership.md).
