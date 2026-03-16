# Language Server

`drat lsp` starts the official Draton Language Server over stdio.

## Current v0 capabilities

- parser and typechecker diagnostics
- hover type information
- go to definition
- document symbols
- workspace symbol lookup across open documents
- basic completion from keywords, top-level names, imports, and visible locals

## Usage

```sh
drat lsp
```

This command is meant to be launched by an editor or LSP client, not typed interactively.

## Editor targets

- VSCode via the minimal extension under `editors/vscode/`
- Neovim or any other client that can start a stdio language server

## Philosophy boundary

The language server reuses the Rust frontend rather than inventing an alternate analysis model. That keeps editor behavior aligned with:

- canonical syntax rules
- strict syntax enforcement
- parser/typechecker diagnostics

## Current limits

Completion is intentionally basic in v0. It is meant to make early Draton editing practical, not to guess aggressively or outgrow the compiler's real understanding.
