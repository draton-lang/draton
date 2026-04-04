# Workspace boundaries

## Authoritative locations

- [`crates/`](../../../../crates/) is the authoritative Rust frontend and tooling implementation.
- [`src/`](../../../../src/) is reserved for the docs site source.
- [`compiler/`](../../../../compiler/) is the only allowed location for current self-host reintroduction work.

## Placement rules

- Do not place compiler implementation in `src/`.
- Do not treat `src/` as a mirror of Rust compiler code.
- Keep self-host work explicit and bounded under `compiler/`.
- Keep Rust frontend behavior authoritative until parity is proven.

## Useful docs

- [`AGENTS.md`](../../../../AGENTS.md)
- [`docs/compiler-architecture.md`](../../../../docs/compiler-architecture.md)
- [`docs/selfhost-canonical-migration-status.md`](../../../../docs/selfhost-canonical-migration-status.md)
