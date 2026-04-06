# Self-Host Type Checker

This directory contains the in-tree self-host typechecker rewrite.

Current boundary:

- type inference truth still comes from `crates/draton-typeck`
- `compiler/driver/typeck_stage.dt` and `compiler/typeck/**` remain the planned self-host stage0 typechecker path
- the hidden Rust stage0 command still routes `typeck` through `host_type_json`, so it remains an oracle path rather than the executable self-host typechecker
- `compiler/typeck/infer/ownership.dt` now writes function ownership summaries and selected expression `use_effect` metadata into the self-host typed program, but full ownership diagnostics parity still follows the Rust authority
- Rust stage0 parity tooling uses `drat selfhost-stage0 typeck --json`
