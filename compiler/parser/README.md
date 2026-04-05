# Self-Host Parser

This directory contains the in-tree canonical Draton parser rewrite.

Current boundary:

- parser grammar and recovery behavior still come from `crates/draton-parser`
- `compiler/driver/parse_stage.dt` remains the planned self-host parser stage0 payload path
- the hidden Rust stage0 command currently reaches parser parity through a generated Draton shim that calls `host_parse_json`, because the full parser stage0 binary does not yet fit the local verification envelope
- Rust stage0 parity tooling uses `drat selfhost-stage0 parse --json`
