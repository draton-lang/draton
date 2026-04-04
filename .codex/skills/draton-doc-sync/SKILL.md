---
name: draton-doc-sync
description: Keep Draton docs, examples, contributor guides, migration notes, and release-facing text aligned with implementation and policy. Use when Codex changes user-visible syntax, CLI behavior, tooling flow, branch policy, release workflow, self-host status, contributor guidance, or any behavior that can drift away from documentation.
---

# Draton Doc Sync

Update the narrowest set of docs that keeps the repository truthful. Do not leave behavior and docs disagreeing across the same task.

## Workflow

1. Read [references/doc-map.md](references/doc-map.md).
2. Identify which public surfaces changed: syntax, tooling, branch policy, release flow, self-host status, install path, or contributor rules.
3. Update only the docs tied to that surface, but update all of them in the same task.
4. If syntax-facing behavior changed, update `docs/syntax-migration.md`.
5. If self-host scope or parity status changed, update `docs/selfhost-canonical-migration-status.md`.
6. If branch or release policy changed, update contributor-facing docs and release workflow docs together.

## Coordination

- Coordinate with `$draton-language-guard` for syntax or philosophy changes.
- Coordinate with `$draton-branch-promotion` for branch-policy text.
- Coordinate with `$draton-release-readiness` for release notes, packaging, and install flow docs.

## Resources

- Load [references/doc-map.md](references/doc-map.md) for the update matrix.
