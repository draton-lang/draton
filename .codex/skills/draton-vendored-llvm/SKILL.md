---
name: draton-vendored-llvm
description: Handle Draton vendored LLVM setup, environment, packaging, and validation workflows. Use when Codex touches scripts/vendor_llvm.py, LLVM bundle environment variables, build failures tied to vendored LLVM, packaging of llvm/, or release smoke tests that depend on the bundled toolchain.
---

# Draton Vendored LLVM

Treat vendored LLVM as an explicit workflow with fetch, environment, build, and packaging steps. Prefer validated repository commands over ad hoc toolchain assumptions.

## Workflow

1. Read [references/llvm-map.md](references/llvm-map.md).
2. Identify whether the task is about fetch, environment, build, packaging, or smoke testing.
3. Use `scripts/recommend_llvm_commands.py <area>` to print the standard command sequence.
4. Keep environment handling explicit with `scripts/vendor_llvm.py`.
5. If release packaging is involved, coordinate with `$draton-release-readiness`.

## Rules

- Do not assume system LLVM is an acceptable silent fallback when the repo expects the vendored flow.
- Keep bundle-related claims aligned with release smoke coverage.

## Resources

- Load [references/llvm-map.md](references/llvm-map.md) for the vendored LLVM map.
- Run `python3 scripts/recommend_llvm_commands.py <area>` for standard commands.
