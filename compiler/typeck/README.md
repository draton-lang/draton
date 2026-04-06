# Self-Host Type Checker

This directory contains the in-tree self-host typechecker rewrite.

Current boundary:

- type inference truth still comes from `crates/draton-typeck`
- `compiler/driver/typeck_stage.dt` and `compiler/typeck/**` remain the planned self-host stage0 typechecker path
- the hidden Rust stage0 command now routes `typeck` through the Draton lexer/parser/typechecker path and normalizes the JSON envelope in Rust
- `compiler/typeck/infer/ownership.dt` now writes function ownership summaries into the self-host typed program, but full ownership diagnostics parity still follows the Rust authority
- Rust stage0 parity tooling uses `drat selfhost-stage0 typeck --json`
