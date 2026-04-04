---
name: draton-workspace-boundaries
description: Respect Draton repository boundaries and sources of truth. Use when Codex must decide whether work belongs in crates/, compiler/, src/, docs/, scripts/, or contributor docs, especially when tasks risk mixing the Rust frontend, docs site, and self-host reintroduction areas.
---

# Draton Workspace Boundaries

Choose the correct part of the repository before editing. This skill prevents implementation work from drifting into the wrong tree.

## Workflow

1. Read [references/boundaries.md](references/boundaries.md).
2. Identify the target area of the task.
3. Apply these default boundaries:
   - `crates/` is the authoritative Rust frontend and tooling implementation.
   - `src/` is for the docs site, not compiler implementation.
   - `compiler/` is the only allowed location for current self-host reintroduction work.
4. If the task crosses areas, keep the Rust frontend authoritative and document the boundary explicitly.

## Coordination

- Use `$draton-selfhost-boundary` when the task touches `compiler/` or self-host parity.
- Use `$draton-doc-sync` if boundary docs must change with the implementation.

## Resources

- Load [references/boundaries.md](references/boundaries.md) for the area map.
