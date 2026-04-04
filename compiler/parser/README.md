# Self-Host Parser

This directory contains the in-tree canonical Draton parser rewrite.

Current boundary:

- parser grammar and recovery behavior still come from `crates/draton-parser`
- stage0 `parse` now runs through the self-host lexer/parser path in `compiler/driver/parse_stage.dt`
- `compiler/driver/parse_stage.dt` is responsible for preserving the Rust-shaped stage0 JSON contract for parser parity
- Rust stage0 parity tooling uses `drat selfhost-stage0 parse --json`
