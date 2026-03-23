---
title: Benchmark snapshots
sidebar_position: 31
---

# Benchmark snapshots

This page covers the benchmark posture for the current compiler and runtime.

Draton now uses Inferred Ownership for safe code. Benchmark discussions in the active documentation should therefore describe compile-time ownership inference, `malloc`/generated-`free` lowering, and runtime services such as scheduling or channels.

Historical pre-migration benchmark artifacts remain in the repository history and under `docs/benchmarks/`, but they are not part of the active runtime story and are intentionally omitted from the current documentation surface.

## Current references

- [Runtime and Ownership](runtime-and-gc.md)
- [Inferred Ownership specification](inferred-ownership-spec.md)

## How to read them

Current performance and behavior discussions should be read against the ownership-era model:

- use the ownership specification for semantic rules
- use the runtime documentation for the active runtime boundary
- use the migration note when historical comparison is needed
