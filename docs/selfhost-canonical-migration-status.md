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

Migration techniques used:

- class fields moved to class-scope `@type`
- local typed bindings moved to function-scope `@type`
- top-level function contracts rewritten from `fn ... -> ...` members to binding-style `name: (...) -> ...`
- executable function definitions stripped of inline parameter and return annotations
- backend entry-layer locals that required typed empty-array initialization moved to function-scope `@type`
- backend emit/layout helper signatures rewritten to canonical file-scope bindings
- backend emit/layout typed empty-array locals moved to function-scope `@type`

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

What remains in the backend slice is now narrower:

- closure files and a few small backend helpers still carry compatibility-form inline syntax in their own source
- those files are no longer blocked by missing canonical-contract semantics in the frontend mirror, but they were not mechanically rewritten in this pass
- the remaining backend debt is therefore mostly source-level canonicalization and any downstream typed-AST cleanup tied to the closure/helper slice

## Skipped Files

The files below still contain deprecated inline syntax after this pass.

### Blocker: Self-Host Semantic Parity Still Missing

These files are coupled to parts of the self-host mirror that still model or implement legacy syntax internally. Rewriting them mechanically would either leave the mirror internally inconsistent or require a much larger coordinated AST/parser/typechecker/codegen refactor.

| File | Exact blocker |
| --- | --- |
| `src/ast/expr.dt` | Expression node constructors and helpers still mirror legacy typed fields and are tightly coupled to `src/typeck/typed_ast/expr.dt` and parser/typechecker consumers. |
| `src/parser/parse/expr.dt` | Parser mirror still implements legacy type-hint and inline signature paths directly in expression parsing helpers. |
| `src/parser/parse/types.dt` | Type parser mirror still assumes older inline syntax entry points and should be migrated together with the rest of `src/parser/parse/*`. |
| `src/typeck/env/env.dt` | Environment layout and helpers still assume older annotation flow from parser/typechecker mirror. |
| `src/typeck/env/scope.dt` | Scope representation is coupled to current self-host typechecker hint propagation and needs coordinated migration with `env.dt`. |
| `src/typeck/exhaust/check.dt` | Exhaustiveness logic still depends on legacy typed structures from the self-host checker mirror. |
| `src/typeck/exhaust/classify.dt` | Pattern classification still relies on older mirrored typed representations. |
| `src/typeck/exhaust/pattern.dt` | Pattern helpers still use legacy typed locals that are coupled to incomplete self-host semantic parity. |
| `src/typeck/infer/expr.dt` | Expression inference still contains additional self-host parity gaps outside the new contract collection path, especially around the broader typed-expression mirror and downstream backend consumers. |
| `src/typeck/infer/subst.dt` | Substitution logic still reflects pre-migration typechecker mirror structures. |
| `src/typeck/infer/unify.dt` | Unification helpers are coupled to the current self-host type representation and checker flow. |
| `src/typeck/typed_ast/expr.dt` | Typed expression mirror still stores many legacy inline-typed fields and constructors consumed throughout the self-host backend. |
| `src/codegen/closure/capture.dt` | Closure capture walker still depends on current typed AST mirror shapes. |
| `src/codegen/closure/emit.dt` | Closure emission still depends on legacy typed AST and checker mirror contracts. |
| `src/codegen/closure/env.dt` | Closure environment layout still follows the current typed AST mirror, not a fully canonicalized one. |
| `src/codegen/error.dt` | Codegen error mirror is still consumed by non-migrated codegen modules and is better migrated with the rest of that slice. |
| `src/codegen/types/descriptor.dt` | Descriptor helpers are tied to the non-migrated codegen/type slices. |
| `src/mono/substitute.dt` | Monomorphization substitution still depends on current self-host typed AST shapes. |

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

What is still not true:

- the self-host mirror is **not yet** at full semantic parity with the Rust frontend for canonical syntax
- `--strict-syntax` still cannot be applied cleanly to the entire `src/` tree
- the remaining debt is now concentrated mostly in the self-host backend closure/helper slice and in the still-unmigrated source text of semantically-unblocked files

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

## Recommended Next Steps

1. Canonicalize the source text of the now-unblocked parser/typechecker/mono files without changing semantics.
2. Migrate the remaining self-host backend closure/helper slice, starting with `src/codegen/closure/capture.dt`, `src/codegen/closure/emit.dt`, and `src/codegen/closure/env.dt`.
3. Re-run `src/main.dt` under stricter focused checks once the closure/helper slice is canonicalized enough to reduce warning noise.
4. Finish large deferred printer/dump modules only after the semantic slices are stable.
