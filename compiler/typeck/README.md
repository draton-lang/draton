# Self-Host Type Checker

This directory is reserved for the self-host Hindley-Milner typechecker rewrite.

Current boundary:

- type inference truth still comes from `crates/draton-typeck`
- ownership inference stays out of the initial Phase 1 self-host scope
- Rust stage0 parity tooling uses `drat selfhost-stage0 typeck --json`
