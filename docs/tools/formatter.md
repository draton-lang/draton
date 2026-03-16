# Formatter

`drat fmt` is the official deterministic formatter for Draton source files.

## Goals

- normalize spacing and indentation
- keep canonical `import { ... } from ...` layout predictable
- format `@type` blocks, class/layer blocks, and function bodies consistently
- stay idempotent and safe for automatic use

## Usage

```sh
drat fmt .
drat fmt src/
drat fmt --check examples/hello.dt
```

`drat fmt` accepts file paths and directories. Directories are scanned recursively for `.dt` files.

## Current v0 behavior

- formatting is deterministic
- `--check` exits non-zero when a file would change
- files with comments are skipped conservatively in v0, because the current pretty-printer does not round-trip comments safely yet

That conservative skip is intentional. The formatter must not silently drop comments or change semantics.

## Repository usage

The repository ships a focused formatting task:

```sh
drat task fmt
```

That task checks a canonical sample subset rather than the full tree, because two dump/printer modules remain intentionally deferred and some comment-bearing files are still handled conservatively.
