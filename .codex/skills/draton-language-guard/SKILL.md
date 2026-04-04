---
name: draton-language-guard
description: Protect Draton canonical language rules, syntax philosophy, and parser-doc-example alignment. Use when Codex changes syntax, parser behavior, typechecker behavior, language examples, migration notes, contributor rules, or any implementation that might introduce competing canonical syntax or drift away from documented Draton language policy.
---

# Draton Language Guard

Read the language policy before editing syntax-facing code. Reject changes that introduce competing canonical syntax, inline type noise as the new default, or docs that disagree with implementation behavior.

## Workflow

1. Read [references/core-docs.md](references/core-docs.md) and open the specific repo docs it points to.
2. Identify whether the task touches parser behavior, typechecker behavior, examples, docs, or contributor guidance.
3. Preserve these invariants:
   - Keep `let` as the canonical variable declaration form.
   - Keep explicit `return` as the canonical control-flow style.
   - Keep `import { ... } from module.path` as the canonical import syntax.
   - Keep `class` as structure and `layer` as capability grouping.
   - Keep `@type` optional and authoritative rather than inline mandatory noise.
4. If behavior changes are user-visible, coordinate with `$draton-doc-sync`.
5. If code changed, coordinate with `$draton-verification`.

## Decision rules

- Prefer the smallest philosophy-preserving fix.
- Treat compatibility syntax as migration support only, not as an equal design direction.
- Update docs and examples in the same task when syntax-facing behavior changes.
- If parser or typechecker behavior contradicts docs, make them converge before considering the task done.

## Resources

- Load [references/core-docs.md](references/core-docs.md) for the canonical document list and anti-drift checklist.
