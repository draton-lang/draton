---
name: draton-release-readiness
description: Prepare and validate Draton release work, preview packaging, smoke tests, and release-facing docs. Use when Codex touches release workflows, packaging scripts, install docs, release notes, preview artifacts, or tasks that need confidence before a tag or public preview update.
---

# Draton Release Readiness

Treat release work as packaging plus proof. Keep release docs, scripts, and smoke expectations aligned.

## Workflow

1. Read [references/release-map.md](references/release-map.md).
2. Identify the affected release surface: workflow, packaging, smoke testing, install docs, or release notes.
3. Update the matching docs and scripts together.
4. Verify the narrowest relevant package or smoke path before calling the work done.
5. Coordinate with `$draton-branch-promotion` if the work affects promotion into `main`.

## Rules

- Keep preview promises explicit and accurate.
- Do not claim runtime or packaging guarantees that smoke tests do not support.

## Resources

- Load [references/release-map.md](references/release-map.md) for the release surface map.
