# Self-Host Type Checker

This directory contains the in-tree self-host typechecker rewrite.

Current boundary:

- type inference truth still comes from `crates/draton-typeck`
- `compiler/driver/typeck_stage.dt` and `compiler/typeck/**` remain the planned self-host stage0 typechecker path
- the hidden Rust stage0 command currently reaches typechecker parity through a generated Draton shim that calls `host_type_json`, because the full typechecker stage0 binary does not yet fit the local verification envelope
- ownership parity is not proven yet and still follows the Rust authority
- Rust stage0 parity tooling uses `drat selfhost-stage0 typeck --json`
