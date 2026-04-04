---
name: draton-review
description: Review Draton changes with a bug-risk-regression mindset. Use when the user asks for a review, when Codex must audit a diff, pull request, parser change, runtime change, docs-policy change, or any update where findings should be prioritized over summaries.
---

# Draton Review

Lead with findings, not overviews. Focus on bugs, behavioral regressions, policy drift, and missing tests before discussing style or summaries.

## Workflow

1. Read [references/review-checklist.md](references/review-checklist.md).
2. Inspect the changed files and identify the real behavioral surface.
3. Look for:
   - correctness bugs
   - regressions against Draton language policy
   - docs or tests falling out of sync
   - release, branch, or memory-model policy violations
4. Present findings first, ordered by severity, with precise file references.
5. If no findings exist, state that clearly and mention residual risks or missing verification.

## Rules

- Do not start with praise or broad summaries.
- Prefer concrete evidence over speculative concern.
- Mention missing or insufficient verification when it affects confidence.

## Resources

- Load [references/review-checklist.md](references/review-checklist.md) before reviewing complex changes.
