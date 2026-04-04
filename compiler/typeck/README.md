# Self-Host Type Checker

This directory contains the in-tree self-host typechecker rewrite.

Current boundary:

- type inference truth still comes from `crates/draton-typeck`
- stage0 `typeck` now runs through `compiler/driver/typeck_stage.dt` and `compiler/typeck/**` instead of `host_type_json`
- ownership parity is not proven yet and still follows the Rust authority
- Rust stage0 parity tooling uses `drat selfhost-stage0 typeck --json`
