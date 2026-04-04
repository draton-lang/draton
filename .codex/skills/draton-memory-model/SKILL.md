---
name: draton-memory-model
description: Protect Draton inferred-ownership memory-model rules. Use when Codex changes runtime lowering, ownership logic, allocation and free behavior, GC-related docs, LLVM IR generation that might reintroduce GC artifacts, or any task that touches memory-management semantics.
---

# Draton Memory Model

Preserve inferred ownership. Do not let safe-code lowering drift back toward GC-specific machinery.

## Workflow

1. Read [references/memory-model.md](references/memory-model.md).
2. Confirm whether the task changes safe-code lowering, ownership inference, runtime docs, or LLVM IR generation.
3. Reject changes that reintroduce GC calls, shadow stack roots, safepoints, or write barriers in the safe path.
4. If memory-model behavior changes are user-visible or architecture-visible, update the matching docs in the same task.
5. Coordinate verification with `$draton-verification`.

## Rules

- Keep inferred ownership as the model.
- Do not reintroduce `draton_gc_*`, `llvm.gcroot`, `llvm_gc_root_chain`, or write-barrier paths in safe-code lowering.
- Keep runtime docs and lowering behavior aligned.

## Resources

- Load [references/memory-model.md](references/memory-model.md) before approving memory-management changes.
