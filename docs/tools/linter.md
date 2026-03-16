# Linter

`drat lint` provides readable, low-heuristic warnings for common Draton issues while respecting the locked canonical syntax rules.

## Current checks

- deprecated inline type syntax
- unused imports
- unreachable code after guaranteed return
- missing explicit returns when a non-`Unit` contract can obviously fall through
- obvious `@type` contract arity mismatches
- lexer and parser errors in the target files

## Usage

```sh
drat lint src/
drat lint examples tests
```

## Output model

`drat lint` is advisory in v0. It prints warnings and errors, but it does not fail the process just because findings exist. That keeps it usable on mixed or migration-era codebases while still surfacing canonical guidance.

## Philosophy boundary

The linter exists to reinforce Draton's readability-first rules:

- it prefers explicit, actionable warnings over clever style scoring
- it treats deprecated syntax as migration pressure, not as an equal style
- it does not invent competing syntax policies

## Repository usage

The repository ships a task for linting the main source subsets:

```sh
drat task lint
```
