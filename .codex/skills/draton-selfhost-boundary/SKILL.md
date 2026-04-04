---
name: draton-selfhost-boundary
description: Keep Draton self-host work scoped, documented, and subordinate to the Rust frontend. Use when Codex touches compiler/, self-host parity plans, migration status docs, or any work that could blur the boundary between current Rust authority and future self-host implementation.
---

# Draton Selfhost Boundary

Treat self-host work as explicitly limited and subordinate until parity is proven. Keep the boundary visible in both code placement and docs.

## Workflow

1. Read [references/selfhost-map.md](references/selfhost-map.md).
2. Confirm the change belongs under `compiler/` rather than `src/` or `crates/`.
3. Preserve Rust frontend behavior as the authority.
4. Update `docs/selfhost-canonical-migration-status.md` if scope, blockers, or parity status changed.
5. Describe blockers using full file paths.

## Rules

- Do not treat `src/` as compiler source.
- Do not move self-host implementation into undocumented locations.
- Keep new self-host scope minimal and justified.

## Resources

- Load [references/selfhost-map.md](references/selfhost-map.md) for boundary checkpoints.
