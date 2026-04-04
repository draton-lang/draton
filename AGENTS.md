# Draton Repository Instructions

This repository defines and protects the Draton language and its tooling. Treat it as a language-engineering repository first, not a general feature playground.

## Source of truth

- The Rust frontend/tooling path is the authoritative implementation.
- The Cargo workspace lives under `crates/`.
- `src/` is reserved for the Docusaurus docs site source, not compiler implementation code.
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

## Branch strategy

- `main` is the stable branch and the most important protected line in the repository.
- Only push to `main` when the code has already proven stable through development, verification, and release-candidate testing.
- `dev` is the active development branch. Ongoing coding, day-to-day fixes, feature work, and integration should land there first.
- `unstable` is the pre-release validation branch. Promote code from `dev` into `unstable` for broader testing before considering `main`.
- The intended promotion flow is `dev` -> `unstable` -> `main`.
- Do not treat `unstable` as a long-term fork or alternate product direction. Its purpose is to validate what may become stable.
- If a change has not been exercised enough to trust it for users, it does not belong on `main`.
- When documenting release or contribution guidance, keep this branch policy consistent across repo docs.

## Local Codex skills

- Repository-local Codex skills live under `.codex/skills/`.
- Prefer these local skills over generic behavior when a task matches their descriptions.
- Use matching local skills implicitly; do not wait for the user to name them explicitly.
- Keep local skills concise, triggerable by real task wording, and aligned with the policies in this file.

## Local Codex tools

- Repository-local Codex tools live under `.codex/tools/`.
- Default to repository-local tools for command execution whenever the action is more than a trivial quick read.
- Prefer guarded local tools over raw shell commands when the task may consume significant time, memory, CPU, or process slots.
- Use raw shell execution directly only for lightweight inspection or when the local tool suite does not cover the job yet.
- Do not spawn uncontrolled long-running processes when a local guarded tool can perform the work instead.
- Use the matching local tooling skill implicitly when command execution strategy matters.

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

- No in-tree self-host compiler source currently ships in this repository.
- Do not treat `src/` as a compiler mirror; it now belongs to the docs site.
- If self-host code is reintroduced, document its location and boundary first, then keep it aligned with the Rust frontend canonical behavior.
- When documenting blockers or exclusions, always use full file paths, not ambiguous basenames.
- `compiler/` is the only allowed location for the current self-host reintroduction work.
- Any code under `compiler/` remains subordinate to the Rust workspace until parity is proven and the migration status docs say otherwise.
- Do not move self-host implementation code into `src/`, `crates/`, or any undocumented location.

## When changing language syntax or tooling

- Make the smallest philosophy-preserving change that solves the problem.
- Update docs, examples, and tests in the same task.
- Update [docs/syntax-migration.md](docs/syntax-migration.md) if syntax-facing behavior changes.
- Update this file or the linked policy docs if repeated mistakes show a policy gap.
- Add or update CI/tests when practical, especially for anti-drift checks.

## When reintroducing self-host code

- Preserve canonical contract semantics already implemented in the Rust frontend.
- Update [docs/selfhost-canonical-migration-status.md](docs/selfhost-canonical-migration-status.md) in the same task.
- Keep any new self-host scope explicit, minimal, and justified.

## Verification expectations

- Prefer targeted verification that exercises the changed language/tooling path.
- For parser/typechecker changes, run the focused tests already used by the repo:
  - `cargo test -p draton-parser --test items`
  - `cargo test -p draton-typeck --test errors`

## Done when

- docs, examples, parser behavior, and tests remain aligned
- no competing syntax style is introduced
- readability-first design is preserved
- any exclusion list stays explicit, minimal, and documented
