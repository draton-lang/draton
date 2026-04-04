---
name: draton-verification
description: Choose and run focused verification for Draton changes. Use when Codex modifies Rust crates, compiler components, parser or typechecker behavior, CLI tooling, release scripts, vendored LLVM workflows, docs that need sanity checks, or any change that should be proven runnable before completion.
---

# Draton Verification

Pick the narrowest verification that still proves the change. Rerun after fixes instead of stopping at the first failure.

## Workflow

1. Read [references/checks.md](references/checks.md).
2. Classify the change area: parser, typechecker, Rust workspace, release, docs-only, vendored LLVM, or branch-policy docs.
3. Use `scripts/recommend_checks.py` to print a focused command set when the area is unclear.
4. Run the smallest relevant command set first, then expand if the change surface is broader.
5. If a command fails, diagnose, fix, and rerun.
6. Report what ran, what passed, and any external blocker that prevented full verification.

## Required checks

- For parser changes, run `cargo test -p draton-parser --test items`.
- For typechecker error behavior, run `cargo test -p draton-typeck --test errors`.
- For behavior tied to vendored LLVM or release packaging, validate the specific script or smoke path affected.
- For docs-only changes, perform at least a consistency pass against the referenced implementation and docs.

## Resources

- Read [references/checks.md](references/checks.md) for the verification matrix.
- Run `python3 scripts/recommend_checks.py <area>...` for a deterministic checklist.
