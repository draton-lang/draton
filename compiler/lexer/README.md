# Self-Host Lexer

This directory is reserved for the canonical Draton lexer rewrite.

Current boundary:

- token and result parity is still defined by `crates/draton-lexer`
- Rust stage0 parity tooling uses `drat selfhost-stage0 lex --json`
- Draton source added here must align with Rust token behavior exactly
