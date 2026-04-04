# Self-Host Parser

This directory contains the in-tree canonical Draton parser rewrite.

Current boundary:

- parser grammar and recovery behavior still come from `crates/draton-parser`
- stage0 `parse` currently bridges through `host_parse_json` in `compiler/driver/pipeline.dt`
- Rust stage0 parity tooling uses `drat selfhost-stage0 parse --json`
