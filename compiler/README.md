# Self-Host Compiler Boundary

`compiler/` is the in-tree location for the current Draton self-host rewrite.

Current boundary:

- `compiler/` is subordinate to the Rust workspace under `crates/`
- `crates/` remains the authoritative implementation until parity is proven
- `src/` remains reserved for the Docusaurus docs site and must not host compiler implementation code

Rules for work under this tree:

- new Draton code uses canonical syntax only
- behavior is ported from Rust without redesign
- any parity mismatch is fixed by aligning `compiler/` with `crates/`
- ownership inference remains out of the initial Phase 1 self-host scope
