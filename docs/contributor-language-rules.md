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

## When reintroducing self-host code

- preserve parity with the Rust frontend canonical behavior
- do not treat `src/` as compiler source; it is reserved for docs-site assets
- do not reintroduce compatibility-form syntax as the default surface
- keep [selfhost-canonical-migration-status.md](selfhost-canonical-migration-status.md) accurate
- document the new self-host location and boundary in the same task

## Current readiness boundary

- The in-tree self-host rewrite lives under `compiler/`, and it remains subordinate to the Rust frontend/tooling path.
- Strict syntax and anti-drift checks still target the Rust frontend/tooling path as the authority.
- Contributors must update [selfhost-canonical-migration-status.md](selfhost-canonical-migration-status.md) when a self-host bridge, blocker, or parity claim changes.
- The old `src/` mirror remains retired and must not be treated as compiler source.

## Review checklist

Before considering a syntax/tooling change done, verify that:

- docs, parser behavior, examples, and tests agree
- no competing syntax style was introduced
- any exclusion list is still small, explicit, and justified
- the change keeps Draton readable first
