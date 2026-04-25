# Self-Host Parser

This directory contains the in-tree canonical Draton parser rewrite.

Current boundary:

- parser grammar and recovery behavior still come from `crates/draton-parser`
- `compiler/driver/pipeline.dt` owns the current bridge-free stage0 parse payload, while `compiler/driver/parse_stage.dt` remains the planned full self-host parser stage0 payload path
- the hidden Rust stage0 command now dispatches parse to Draton code with `bridge.builtin = null`; representative parser parity is gated by `crates/draton-parser/tests/selfhost_parity.rs`
- Rust stage0 parity tooling uses `drat selfhost-stage0 parse --json`
