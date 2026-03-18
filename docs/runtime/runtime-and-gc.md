---
title: Runtime and GC
sidebar_position: 30
---

# Runtime and GC

Draton’s runtime is the low-level layer that makes the compiled language executable in practice. It is not just a bag of helper functions. It provides memory management, runtime ABI, and other low-level services that the compiler expects.

## Runtime role

The compiler pipeline ends in native code linked against the Draton runtime. That means runtime behavior is part of the real language experience, not just an implementation detail hidden from users.

Key responsibilities include:

- GC and allocation behavior
- runtime string and array helpers
- output/input builtins
- panic/runtime support
- scheduling and concurrency primitives where needed

## GC state

The runtime ships with a generational GC that has been significantly upgraded from its earlier state.

The current shape includes:

- young allocation pool
- old-generation reuse and coalescing
- incremental major collection
- mutator assist paths
- background major progress groundwork
- telemetry, scorecards, and public benchmark artifacts

## Why the GC docs are public

Draton does not hide GC status behind vague claims. The repo publishes scorecards and comparison artifacts so contributors can see:

- what improved
- what still regresses
- where performance still trails OCaml in benchmarked scenarios

This is deliberate. It keeps optimization work grounded in measurements rather than slogans.

## Benchmarks and current truth

Use these resources together:

- [GC scorecard](../gc-scorecard.md)
- [Benchmarks index](./benchmarks.md)

Those docs track both strengths and current weak spots, including scenarios where Draton still loses to OCaml.

## Runtime boundaries that matter to docs

The documentation must not claim:

- that GC is “finished”
- that Draton already beats OCaml everywhere
- that self-host bootstrap is fully green if the tracked blocker still exists

The point of the runtime docs is to be operationally truthful.
