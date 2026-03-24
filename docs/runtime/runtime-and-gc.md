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

## Runtime feature layers

`crates/draton-runtime` now builds as layered runtime variants instead of one fixed host-only archive.

- default build enables `scheduler`, `std-io`, and `host-compiler`
- `scheduler` pulls in the threaded scheduler and channel implementation
- `std-io` enables hosted stdin/stdout/stderr and file builtins
- `host-compiler` enables frontend-dependent helpers used by `drat` and bootstrap flows
- `coop-scheduler` enables a single-threaded cooperative scheduler intended for bare-metal style targets

That keeps hosted builds unchanged while allowing smaller runtime archives for constrained targets.

## Bare-Metal Runtime Builds

For a minimal runtime archive without OS threading, filesystem helpers, or host-compiler helpers:

```bash
cargo build -p draton-runtime --no-default-features --features coop-scheduler
```

`drat build` already supports swapping the runtime archive without changing codegen:

```bash
DRATON_RUNTIME_LIB=/abs/path/to/libdraton_runtime.a drat build path/to/main.dt
```

If your target provides its own runtime stubs or allocator wiring, skip the bundled runtime entirely:

```bash
DRATON_SKIP_RUNTIME_LINK=1 drat build path/to/main.dt
```

For bare-metal integrations, provide a platform implementation before using runtime I/O or panic helpers so the runtime can route stdout, stderr, and panic halt behavior to the target environment.

For the active model, pair this document with the [Inferred Ownership specification](inferred-ownership-spec.md). For historical migration context, use [migration-gc-to-inferred-ownership.md](migration-gc-to-inferred-ownership.md).
