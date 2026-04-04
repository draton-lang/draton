# Self-host boundary

## Hard limits

- No in-tree self-host compiler source currently ships as the authoritative implementation.
- `compiler/` is the only allowed location for current self-host reintroduction work.
- `crates/` remains authoritative until parity is proven.
- `src/` belongs to the docs site, not compiler implementation.

## Required doc sync

If self-host scope or parity status changes, update:

- [`docs/selfhost-canonical-migration-status.md`](../../../../docs/selfhost-canonical-migration-status.md)
- [`docs/compiler-architecture.md`](../../../../docs/compiler-architecture.md) when the architectural boundary changes materially

## Reporting rule

Use full file paths when documenting blockers, gaps, or exclusions.
