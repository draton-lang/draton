# Self-Host Lexer

This directory contains the current canonical Draton lexer rewrite.

Current boundary:

- token and result parity is still defined by `crates/draton-lexer`
- Rust stage0 parity tooling uses `drat selfhost-stage0 lex --json`
- `compiler/driver/pipeline.dt` currently uses this lexer for `lex_json` without a dedicated host lex bridge
- Draton source added here must align with Rust token behavior exactly
