# Self-Host Canonical Migration Status

This document tracks the current state of canonical-syntax migration for the self-host mirror under `src/`.

Scope audited in this pass:

- `src/ast/`
- `src/parser/parse/`
- `src/typeck/`
- `src/codegen/`
- `src/lexer/`
- `src/mono/`

Canonical syntax in scope:

- `let x = ...`
- explicit `return`
- `@type { name: Type }` at file, class, layer, interface, and function scope
- brace imports

Deprecated syntax targeted in this pass:

- `let x: T = value`
- `fn f(a: T)`
- `fn f(...) -> T`

## Migrated In This Pass

The following self-host files were migrated safely to canonical syntax in this pass:

- `src/ast/program.dt`
- `src/ast/stmt.dt`
- `src/ast/types.dt`
- `src/lexer/cursor.dt`
- `src/lexer/error.dt`
- `src/lexer/lexer.dt`
- `src/lexer/token.dt`
- `src/lexer/lit/number.dt`
- `src/lexer/lit/string.dt`
- `src/typeck/error.dt`
- `src/typeck/typed_ast/program.dt`
- `src/typeck/types/row.dt`
- `src/typeck/types/scheme.dt`
- `src/typeck/types/ty.dt`
- `src/codegen/codegen.dt`
- `src/codegen/emit/expr.dt`
- `src/codegen/emit/item.dt`
- `src/codegen/emit/stmt.dt`
- `src/codegen/layout/class.dt`
- `src/codegen/layout/vtable.dt`
- `src/codegen/closure/capture.dt`
- `src/codegen/closure/emit.dt`
- `src/codegen/closure/env.dt`
- `src/parser/parse/types.dt`
- `src/typeck/env/env.dt`
- `src/typeck/env/scope.dt`
- `src/typeck/exhaust/check.dt`
- `src/typeck/exhaust/classify.dt`
- `src/typeck/infer/subst.dt`
- `src/typeck/infer/unify.dt`
- `src/mono/substitute.dt`

Migration techniques used:

- class fields moved to class-scope `@type`
- local typed bindings moved to function-scope `@type`
- top-level function contracts rewritten from `fn ... -> ...` members to binding-style `name: (...) -> ...`
- executable function definitions stripped of inline parameter and return annotations
- backend entry-layer locals that required typed empty-array initialization moved to function-scope `@type`
- backend emit/layout helper signatures rewritten to canonical file-scope bindings
- backend emit/layout typed empty-array locals moved to function-scope `@type`
- backend closure/helper signatures rewritten to canonical file-scope bindings
- backend closure/helper typed empty-array locals moved to function-scope `@type`
- parser/typechecker support helpers rewritten to canonical file-scope bindings
- environment, exhaustiveness, substitution, and monomorphization locals moved to function-scope `@type`

## Semantic Parity Completed In This Pass

The following previously blocked self-host files are no longer blocked by missing canonical-contract semantics:

- `src/ast/item.dt`
- `src/parser/parse/item.dt`
- `src/parser/parse/stmt.dt`
- `src/typeck/infer/item.dt`
- `src/typeck/infer/stmt.dt`
- `src/typeck/typed_ast/item.dt`
- `src/typeck/typed_ast/stmt.dt`
- `src/mono/collector.dt`

What was completed for this cluster:

- interface bodies now accept self-host `@type` blocks
- function bodies now accept self-host `@type` statements
- self-host type inference now collects function-local binding hints from statement `@type` blocks
- self-host interface predeclaration and inference now consume interface binding-style contracts and prepend the implicit receiver internally
- typed AST and monomorphization mirrors now carry and tolerate the new statement and interface-contract shapes

Important status note:

- these files are now semantically unblocked, but several of them still contain compatibility-form inline type syntax in their own source
- the remaining work on those files is now mostly mechanical canonicalization, not missing parser or typechecker capability
- that mechanical rewrite was intentionally left out of this pass to keep the semantic change reviewable

## Backend Entry Layer Completed In This Pass

The main self-host backend/codegen blocker has been reduced at the entry layer:

- `src/codegen/codegen.dt` no longer depends on legacy inline syntax in its own source
- the file now uses canonical class-scope `@type`, file-scope contract bindings, and function-scope `@type` for typed empty-array locals
- canonical contract data flowing through typed items no longer required backend entry-point changes beyond the already-completed parser/typechecker parity work

The backend slice is now substantially cleaner:

- the self-host codegen entry, emit/layout, and closure/helper files have all been migrated to canonical syntax where safe
- no remaining `src/codegen/*.dt` file in this audit is blocked primarily by missing canonical-contract semantics
- any remaining self-host migration debt now sits outside the backend slice

## Skipped Files

The files below still contain deprecated inline syntax after this pass.

### Blocker: Self-Host Semantic Coupling Still Present

These files are still tightly coupled to older mirrored expression/typechecker structures. A purely mechanical rewrite would risk desynchronizing the self-host mirror from the Rust frontend behavior.

| File | Exact blocker |
| --- | --- |
| `src/ast/expr.dt` | Expression node constructors and helpers still mirror legacy typed fields and are tightly coupled to `src/typeck/typed_ast/expr.dt` and parser/typechecker consumers. |
| `src/parser/parse/expr.dt` | Parser mirror still implements legacy type-hint and inline signature paths directly in expression parsing helpers. |
| `src/typeck/infer/expr.dt` | Expression inference still contains additional self-host parity gaps outside the new contract collection path, especially around the broader typed-expression mirror and downstream backend consumers. |

### Blocker: Mechanical Cleanup On Semantically-Unblocked Files

These files are no longer blocked by missing canonical-contract semantics, but they still contain compatibility-form inline syntax in their own source. The remaining work is primarily source-level rewrite, kept separate here to avoid broad risky churn.

| File | Exact blocker |
| --- | --- |
| `src/ast/item.dt` | Large AST constructor/helper surface still uses legacy inline signatures; migration is mostly mechanical but wide. |
| `src/parser/parse/item.dt` | Parser item helpers are semantically unblocked but still have many compatibility-form function signatures. |
| `src/parser/parse/stmt.dt` | Statement parser mirror is semantically unblocked but still source-level legacy syntax heavy. |
| `src/typeck/infer/item.dt` | Item inference is semantically unblocked but still contains a large amount of compatibility-form syntax. |
| `src/typeck/infer/stmt.dt` | Statement inference is semantically unblocked but still source-level legacy syntax heavy. |
| `src/mono/collector.dt` | Monomorphization collector is semantically unblocked but still contains broad compatibility-form signatures. |

Files that no longer appear here:

- `src/parser/parse/types.dt` was mechanically canonicalized in this pass
- `src/typeck/env/env.dt` was mechanically canonicalized in this pass
- `src/typeck/env/scope.dt` was mechanically canonicalized in this pass
- `src/typeck/exhaust/check.dt` was mechanically canonicalized in this pass
- `src/typeck/exhaust/classify.dt` was mechanically canonicalized in this pass
- `src/typeck/infer/subst.dt` was mechanically canonicalized in this pass
- `src/typeck/infer/unify.dt` was mechanically canonicalized in this pass
- `src/mono/substitute.dt` was mechanically canonicalized in this pass
- `src/typeck/typed_ast/expr.dt` currently contains no remaining deprecated inline syntax in this audit
- `src/typeck/exhaust/pattern.dt` currently contains no remaining deprecated inline syntax in this audit

### Blocker: Another Specific Reason

These files are mechanically migratable, but were deferred in this pass because they are very large pretty-printer / dump modules. Rewriting hundreds of signature lines at once would create review noise without reducing the core semantic gap.

| File | Exact blocker |
| --- | --- |
| `src/ast/dump.dt` | Large printer module with more than a thousand lines of constructor-dump helpers; deferred to keep this pass reviewable. |
| `src/typeck/dump.dt` | Large printer module with similar mechanical churn and low semantic payoff compared with parser/typechecker/codegen core files. |

### Blocker: Generic Contract Syntax Still Missing

No remaining `src/` files in this scan were blocked primarily by generic-contract syntax. The dominant blocker is still incomplete self-host semantic parity.

## Current Parity Summary

After this pass, the self-host mirror is materially closer to canonical syntax in the following areas:

- core AST support files
- foundational type model helpers
- lexer cursor/token/error/result helpers
- self-host-facing utility modules that previously depended only on local typed bindings or class field annotations
- self-host parser/typechecker/mono handling for function-scope and interface-scope canonical `@type`
- self-host backend entry-layer codegen state and predeclaration plumbing
- self-host backend emit/layout handling and source-level canonical syntax in the core emit path
- self-host backend closure capture/emission/environment helpers and the rest of the codegen slice
- parser/type parser helpers, environment plumbing, exhaustiveness support, substitution, unification, and monomorphization substitution support

What is still not true:

- the self-host mirror is **not yet** at full semantic parity with the Rust frontend for canonical syntax
- `--strict-syntax` still cannot be applied cleanly to the entire `src/` tree
- the remaining debt is now concentrated in expression-heavy frontend/typechecker files plus large mechanically deferred dump/printer modules

In practice, the repository is now in an intermediate state:

- the Rust frontend/tooling path is authoritative for canonical syntax
- the self-host source tree has been partially canonicalized where safe
- the remaining migration work is no longer mostly mechanical; it requires coordinated mirror updates across AST, parser, typechecker, and backend layers

## Verification Run In This Pass

- `cargo test -p draton-parser --test items -p draton-typeck --test errors`
- `cargo run -p drat -- build --strict-syntax tests/programs/gc/stress_linked_list.dt -o /tmp/draton_gc_linked_list_strict`
- `/tmp/draton_gc_linked_list_strict` returned exit code `50` as expected
- `cargo run -p drat -- build --strict-syntax /tmp/draton_selfhost_semantic_parity.dt -o /tmp/draton_selfhost_semantic_parity_out`
- `/tmp/draton_selfhost_semantic_parity_out` returned exit code `7`
- `cargo run -p drat -- build src/main.dt -o /tmp/draton_selfhost_semantic_batch`
- `cargo run -p drat -- build --strict-syntax /tmp/draton_selfhost_semantic_parity.dt -o /tmp/draton_selfhost_emit_slice_out`
- `/tmp/draton_selfhost_emit_slice_out` returned exit code `7`
- `cargo run -p drat -- build src/main.dt -o /tmp/draton_selfhost_emit_slice_batch`
- `cargo run -p drat -- build --strict-syntax tests/programs/compile/51_lambda_capture.dt -o /tmp/draton_lambda_capture_strict`
- `/tmp/draton_lambda_capture_strict` returned exit code `15`
- `cargo run -p drat -- build src/main.dt -o /tmp/draton_selfhost_closure_slice`
- `cargo run -p drat -- build --strict-syntax tests/programs/compile/42_interface_upcast_parent_impl.dt -o /tmp/draton_frontend_subset_strict`
- `/tmp/draton_frontend_subset_strict` returned exit code `7`
- `cargo run -p drat -- build src/main.dt -o /tmp/draton_selfhost_frontend_subset`

## Recommended Next Steps

1. Canonicalize the source text of the remaining semantically-unblocked frontend/typechecker files, starting with `src/parser/parse/item.dt`, `src/parser/parse/stmt.dt`, `src/typeck/infer/item.dt`, and `src/typeck/infer/stmt.dt`.
2. Tackle the expression-heavy semantic-coupling trio `src/ast/expr.dt`, `src/parser/parse/expr.dt`, and `src/typeck/infer/expr.dt` as one coordinated pass.
3. Finish large deferred printer/dump modules only after the expression-heavy semantic slice is stable.

## Near-Final State

The self-host mirror is now close to full canonical syntax parity in the backend path:

- `src/codegen/` no longer contains repository-known blockers caused by deprecated inline syntax in the audited files
- a focused strict-canonical CI subset is now practical for canonical fixtures plus selected self-host-facing builds, because the backend and major support helpers are syntax-clean
- the main remaining blockers are concentrated in a smaller frontend/typechecker expression slice plus a limited number of semantically-unblocked files still awaiting mechanical rewrite
- the remaining skipped set is therefore mostly frontend cleanup plus a small amount of intentionally deferred dump/printer churn
