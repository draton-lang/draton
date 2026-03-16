# Contributor Language Rules

This document defines the anti-drift guardrails for contributors and coding agents.

For philosophy, see [language-manifesto.md](language-manifesto.md). For exact syntax shape, see [canonical-syntax-rules.md](canonical-syntax-rules.md).

## Hard rules

Do not:

- reintroduce inline variable types as canonical style
- reintroduce typed function parameters as canonical style
- reintroduce inline return types as canonical style
- add a second canonical import style
- weaken explicit `return` into an implicit-return-only philosophy
- turn `@type` into mandatory inline noise
- let docs/examples drift away from parser/typechecker behavior
- describe blockers using ambiguous basenames when full file paths are available

## Compatibility policy

- Compatibility syntax may remain temporarily for migration support.
- Compatibility syntax is not a second design philosophy.
- If compatibility is preserved, docs must say so explicitly and must still present canonical syntax first.

## When changing syntax-facing behavior

- update docs and examples in the same task
- update [syntax-migration.md](syntax-migration.md) when behavior visible to users changes
- update tests or CI checks when practical
- preserve readability-first design even in local fixes

## When changing self-host code

- preserve parity with the Rust frontend canonical behavior
- do not reintroduce compatibility-form syntax in migrated files
- keep [selfhost-canonical-migration-status.md](selfhost-canonical-migration-status.md) accurate
- treat these files as intentionally excluded until explicitly changed:
  - `src/ast/dump.dt`
  - `src/typeck/dump.dt`

## Current readiness boundary

- Executable/compiler-path self-host canonical migration is complete.
- The strict self-host subset CI protects the migrated executable self-host path.
- Full-tree strict self-host CI still requires canonicalizing or intentionally retiring:
  - `src/ast/dump.dt`
  - `src/typeck/dump.dt`

## Review checklist

Before considering a syntax/tooling change done, verify that:

- docs, parser behavior, examples, and tests agree
- no competing syntax style was introduced
- any exclusion list is still small, explicit, and justified
- the change keeps Draton readable first

