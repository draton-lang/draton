# Contributing to Draton

Draton accepts contributions, but the repository is intentionally strict about branch flow and language/tooling stability.

## Branch workflow

- `main` is the stable branch.
- `main` is reserved for changes that are already proven stable.
- Do not push experimental, partially verified, or still-churning work to `main`.
- `dev` is the primary development branch.
- Ongoing implementation, routine fixes, refactors, and feature work should be pushed to `dev`.
- `unstable` is the release-candidate and testing branch.
- Promote changes from `dev` to `unstable` when they are ready for wider validation.
- Only promote from `unstable` to `main` after the code has been tested enough to justify calling it stable.

Promotion path:

```text
dev -> unstable -> main
```

## Practical expectations

- Start normal contribution work from `dev`.
- Use `unstable` to validate integrated changes before they become stable.
- Treat `main` as the branch that should stay dependable for downstream users, release preparation, and public consumption.
- If you are unsure whether a change is stable enough for `main`, it is not ready for `main`.

## Language and tooling guardrails

Read these before opening syntax-facing or tooling-heavy changes:

- [AGENTS.md](../AGENTS.md)
- [docs/contributor-language-rules.md](../docs/contributor-language-rules.md)
- [docs/canonical-syntax-rules.md](../docs/canonical-syntax-rules.md)
- [docs/compiler-architecture.md](../docs/compiler-architecture.md)
- [docs/release-workflow.md](../docs/release-workflow.md)
