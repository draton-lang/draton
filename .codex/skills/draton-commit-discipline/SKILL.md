---
name: draton-commit-discipline
description: Keep Draton commit history disciplined and policy-compliant. Use when Codex has completed a meaningful change, needs to split work into logical commits, must choose commit boundaries, or must write clear commit messages that reflect repository rules.
---

# Draton Commit Discipline

Commit when the work reaches a stable, runnable state. Keep each commit to one logical unit and make the message specific.

## Workflow

1. Read [references/commit-policy.md](references/commit-policy.md).
2. Decide whether the current work is one logical unit or needs splitting.
3. Stage only the files that belong to that unit.
4. Write a descriptive message using `type: summary`.
5. Commit immediately once the changed code or docs are stable.

## Rules

- Do not leave important work uncommitted.
- Do not use vague messages such as `fix`, `update`, `misc`, or `wip`.
- Do not combine unrelated changes into one commit unless the user explicitly asks for that.

## Resources

- Load [references/commit-policy.md](references/commit-policy.md) for exact commit guardrails.
