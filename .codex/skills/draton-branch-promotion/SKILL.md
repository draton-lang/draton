---
name: draton-branch-promotion
description: Apply Draton branch policy and promotion flow. Use when Codex works on branch creation, branch cleanup, merge strategy, contributor branch guidance, release-candidate promotion, or any task that decides whether work belongs on dev, unstable, or main.
---

# Draton Branch Promotion

Follow the repository's long-lived branch model strictly. Keep `main` stable, use `dev` for ongoing development, and use `unstable` as the release-candidate validation step.

## Workflow

1. Read [references/branch-policy.md](references/branch-policy.md).
2. Determine which stage the work belongs to:
   - `dev` for active implementation and frequent pushes.
   - `unstable` for promoted code under broader testing.
   - `main` only for changes already proven stable.
3. If docs mention branch flow, keep `AGENTS.md`, `.github/CONTRIBUTING.md`, `README.md`, and `docs/release-workflow.md` aligned.
4. If branch cleanup or promotion is requested, protect `main` first and make the path `dev -> unstable -> main` explicit.

## Rules

- Do not use `main` as a discovery branch.
- If a change is still churning, it belongs on `dev`.
- If a change needs broader pre-release validation, promote it to `unstable` before `main`.

## Resources

- Load [references/branch-policy.md](references/branch-policy.md) for the exact policy text to mirror.
