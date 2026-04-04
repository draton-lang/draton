# Self-Host Compiler Boundary

`compiler/` is the in-tree location for the current Draton self-host rewrite.

Current boundary:

- `compiler/` is subordinate to the Rust workspace under `crates/`
- `crates/` remains the authoritative implementation until parity is proven
- `src/` remains reserved for the Docusaurus docs site and must not host compiler implementation code
- `compiler/main.dt` is the current stage0 entrypoint
- `compiler/driver/pipeline.dt` currently runs `lex_json`, `parse_json`, and `typeck_json` in Draton and still bridges `build_json` through a Rust host builtin
- current status tracking lives in `docs/selfhost-canonical-migration-status.md`

Rules for work under this tree:

- new Draton code uses canonical syntax only
- behavior is ported from Rust without redesign
- any parity mismatch is fixed by aligning `compiler/` with `crates/`
- ownership parity is not proven yet and must not be claimed as complete
