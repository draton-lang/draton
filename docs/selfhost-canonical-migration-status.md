---
title: Self-host status
sidebar_position: 35
---

# Self-host status

The historical self-host compiler mirror under `src/` was intentionally removed while the next rewrite is prepared.

## Current state

- no in-tree self-host compiler source is currently shipped
- `src/` now belongs to the Docusaurus docs site source (`src/pages`, `src/css`)
- the Rust workspace under `crates/` is the only authoritative compiler/tooling implementation in the repository today

## Why this changed

The old tree had become a source of drift and cleanup overhead while no longer serving as the active implementation path. Removing it makes room for a fresh self-host compiler design without pretending the old mirror is still current.

## Guidance for future reintroduction

- choose a dedicated location and document it before landing compiler code
- keep the Rust frontend/tooling path as the source of truth until parity is proven
- adopt canonical syntax from the start instead of reviving compatibility-form debt
- update [AGENTS.md](https://github.com/draton-lang/draton/blob/main/AGENTS.md), [compiler-architecture.md](compiler-architecture.md), and this file in the same task

## Historical record

The old migration details remain available in Git history. This document now tracks the reset boundary rather than the retired tree's cleanup checklist.
