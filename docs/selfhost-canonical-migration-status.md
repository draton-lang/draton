---
title: Self-host status
sidebar_position: 35
---

# Self-host status

The historical self-host compiler mirror under `src/` was intentionally removed while the next rewrite was prepared.

## Current state

- a new in-tree self-host rewrite started on March 25, 2026 under `compiler/`
- `compiler/` is the only approved self-host location for the current rewrite
- `src/` now belongs to the Docusaurus docs site source (`src/pages`, `src/css`)
- the Rust workspace under `crates/` remains the only authoritative compiler/tooling implementation until parity is proven

## Current boundary

- `compiler/` is a subordinate self-host tree for rewrite and parity work
- `crates/` remains the source of truth for syntax, parser, typechecker, codegen, CLI, and runtime behavior
- any mismatch between `compiler/` and `crates/` is resolved by aligning `compiler/` to Rust, not by redesigning the language
- ownership inference for the self-host compiler remains deferred beyond the initial Phase 1 rewrite scope

## Why this changed

The old tree had become a source of drift and cleanup overhead while no longer serving as the active implementation path. Removing it made room for a fresh self-host compiler design without pretending the old mirror was still current.

## Guidance for future reintroduction

- keep `compiler/` explicit and documented before landing more self-host code
- keep the Rust frontend/tooling path as the source of truth until parity is proven
- adopt canonical syntax from the start instead of reviving compatibility-form debt
- update [AGENTS.md](https://github.com/draton-lang/draton/blob/main/AGENTS.md), [compiler-architecture.md](compiler-architecture.md), and this file in the same task

## Historical record

The old migration details remain available in Git history. This document now tracks the reset boundary rather than the retired tree's cleanup checklist.
