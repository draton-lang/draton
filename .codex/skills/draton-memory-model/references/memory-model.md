# Memory model

## Core rule

Draton uses Inferred Ownership.

## Never reintroduce

- `draton_gc_*`
- `llvm.gcroot`
- `llvm_gc_root_chain`
- shadow stack references
- safepoint logic
- write-barrier paths in safe-code lowering

## Related docs

- [`AGENTS.md`](../../../../AGENTS.md)
- [`docs/runtime/inferred-ownership-spec.md`](../../../../docs/runtime/inferred-ownership-spec.md)
- [`docs/runtime/migration-gc-to-inferred-ownership.md`](../../../../docs/runtime/migration-gc-to-inferred-ownership.md)
- [`docs/runtime/runtime-and-gc.md`](../../../../docs/runtime/runtime-and-gc.md)
