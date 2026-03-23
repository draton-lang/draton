# Draton Repository Instructions

This repository defines and protects the Draton language, its tooling, and its self-host mirror. Treat it as a language-engineering repository first, not a general feature playground.

## Source of truth

- The Rust frontend/tooling path is the authoritative implementation.
- The self-host mirror under `src/` must preserve parity with that canonical behavior.
- Read these docs before making syntax-facing changes:
  - [docs/language-manifesto.md](docs/language-manifesto.md)
  - [docs/language-architecture.md](docs/language-architecture.md)
  - [docs/language-class-diagram.md](docs/language-class-diagram.md)
  - [docs/language-analyst-artifact.md](docs/language-analyst-artifact.md)
  - [docs/compiler-architecture.md](docs/compiler-architecture.md)
  - [docs/canonical-syntax-rules.md](docs/canonical-syntax-rules.md)
  - [docs/contributor-language-rules.md](docs/contributor-language-rules.md)
  - [docs/syntax-migration.md](docs/syntax-migration.md)
  - [docs/selfhost-canonical-migration-status.md](docs/selfhost-canonical-migration-status.md)

## Non-negotiable language rules

- Preserve readability-first code.
- Code expresses behavior; `@type` expresses contracts.
- Keep `let` as the canonical variable declaration form.
- Keep explicit `return` as the canonical control-flow style.
- Keep `import { ... } from module.path` as the canonical import syntax.
- Keep `class` as structure and `layer` as capability grouping.
- Keep `@type { name: Type }` optional and authoritative; never turn it into mandatory inline noise.
- Compatibility syntax exists only for migration support, not as a competing design direction.

## Never introduce

- inline variable types as canonical syntax
- typed function parameters as canonical syntax
- inline return types as canonical syntax
- a second canonical import style
- an implicit-return-only philosophy
- syntax or docs that contradict the class/layer model
- docs or examples that disagree with parser/typechecker behavior

## Memory model

- Draton uses Inferred Ownership.
- Do not introduce GC calls, shadow stack references, or safepoint logic.
- Do not reintroduce `draton_gc_*`, `llvm.gcroot`, `llvm_gc_root_chain`, or write-barrier paths in safe-code lowering.

## Self-host boundary

- Do not reintroduce compatibility-form syntax in already migrated self-host files.
- Keep parity with the Rust frontend canonical behavior when touching `src/`.
- Do not claim full-tree strict self-host coverage while these files remain excluded:
  - `src/ast/dump.dt`
  - `src/typeck/dump.dt`
- When documenting blockers or exclusions, always use full file paths, not ambiguous basenames.

## When changing language syntax or tooling

- Make the smallest philosophy-preserving change that solves the problem.
- Update docs, examples, and tests in the same task.
- Update [docs/syntax-migration.md](docs/syntax-migration.md) if syntax-facing behavior changes.
- Update this file or the linked policy docs if repeated mistakes show a policy gap.
- Add or update CI/tests when practical, especially for anti-drift checks.

## When changing self-host code

- Preserve canonical contract semantics already implemented in the Rust frontend.
- Keep [docs/selfhost-canonical-migration-status.md](docs/selfhost-canonical-migration-status.md) accurate.
- Treat `src/ast/dump.dt` and `src/typeck/dump.dt` as intentionally deferred until explicitly changed.
- Keep the strict self-host subset small, explicit, and justified.

## Verification expectations

- Prefer targeted verification that exercises the changed language/tooling path.
- For syntax/policy drift in the self-host mirror, run:
  - `python3 tools/check_selfhost_strict_subset.py`
- For parser/typechecker changes, run the focused tests already used by the repo:
  - `cargo test -p draton-parser --test items`
  - `cargo test -p draton-typeck --test errors`

## Done when

- docs, examples, parser behavior, and tests remain aligned
- no competing syntax style is introduced
- readability-first design is preserved
- any exclusion list stays explicit, minimal, and documented
