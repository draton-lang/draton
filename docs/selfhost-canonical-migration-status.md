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

Migration techniques used:

- class fields moved to class-scope `@type`
- local typed bindings moved to function-scope `@type`
- top-level function contracts rewritten from `fn ... -> ...` members to binding-style `name: (...) -> ...`
- executable function definitions stripped of inline parameter and return annotations

## Skipped Files

The files below still contain deprecated inline syntax after this pass.

### Blocker: Self-Host Semantic Parity Still Missing

These files are coupled to parts of the self-host mirror that still model or implement legacy syntax internally. Rewriting them mechanically would either leave the mirror internally inconsistent or require a much larger coordinated AST/parser/typechecker/codegen refactor.

| File | Exact blocker |
| --- | --- |
| `src/ast/expr.dt` | Expression node constructors and helpers still mirror legacy typed fields and are tightly coupled to `src/typeck/typed_ast/expr.dt` and parser/typechecker consumers. |
| `src/ast/item.dt` | Item/class/interface mirror still needs coordinated updates for interface `@type` blocks and canonical member-contract storage across the rest of the self-host pipeline. |
| `src/parser/parse/expr.dt` | Parser mirror still implements legacy type-hint and inline signature paths directly in expression parsing helpers. |
| `src/parser/parse/item.dt` | Parser mirror still builds old item/function/class/interface shapes; canonicalizing source alone would not complete parser-behavior parity. |
| `src/parser/parse/stmt.dt` | Statement parser mirror still carries legacy typed-let parsing paths and has not been updated end-to-end to self-host canonical block semantics. |
| `src/parser/parse/types.dt` | Type parser mirror still assumes older inline syntax entry points and should be migrated together with the rest of `src/parser/parse/*`. |
| `src/typeck/env/env.dt` | Environment layout and helpers still assume older annotation flow from parser/typechecker mirror. |
| `src/typeck/env/scope.dt` | Scope representation is coupled to current self-host typechecker hint propagation and needs coordinated migration with `env.dt`. |
| `src/typeck/exhaust/check.dt` | Exhaustiveness logic still depends on legacy typed structures from the self-host checker mirror. |
| `src/typeck/exhaust/classify.dt` | Pattern classification still relies on older mirrored typed representations. |
| `src/typeck/exhaust/pattern.dt` | Pattern helpers still use legacy typed locals that are coupled to incomplete self-host semantic parity. |
| `src/typeck/infer/expr.dt` | Expression inference is one of the main self-host semantic gaps; local and interface `@type` semantics are not mirrored here yet. |
| `src/typeck/infer/item.dt` | Item inference still predeclares and consumes legacy annotation shapes across classes, interfaces, and methods. |
| `src/typeck/infer/stmt.dt` | Statement inference still assumes legacy typed-let and inline signature flow from the self-host parser mirror. |
| `src/typeck/infer/subst.dt` | Substitution logic still reflects pre-migration typechecker mirror structures. |
| `src/typeck/infer/unify.dt` | Unification helpers are coupled to the current self-host type representation and checker flow. |
| `src/typeck/typed_ast/expr.dt` | Typed expression mirror still stores many legacy inline-typed fields and constructors consumed throughout the self-host backend. |
| `src/typeck/typed_ast/item.dt` | Typed item mirror still needs coordinated canonical contract storage for classes, interfaces, and type blocks. |
| `src/typeck/typed_ast/stmt.dt` | Typed statement mirror still carries legacy inline field annotations and would need coordinated downstream updates. |
| `src/codegen/closure/capture.dt` | Closure capture walker still depends on current typed AST mirror shapes. |
| `src/codegen/closure/emit.dt` | Closure emission still depends on legacy typed AST and checker mirror contracts. |
| `src/codegen/closure/env.dt` | Closure environment layout still follows the current typed AST mirror, not a fully canonicalized one. |
| `src/codegen/codegen.dt` | Core codegen state still contains many inline-typed fields and helper contracts tied to incomplete self-host semantic parity. |
| `src/codegen/emit/expr.dt` | Expression emission still depends on legacy typed AST/value-contract shapes. |
| `src/codegen/emit/item.dt` | Item emission is coupled to current codegen and typed item mirror structures. |
| `src/codegen/emit/stmt.dt` | Statement emission still follows the current typed statement mirror. |
| `src/codegen/error.dt` | Codegen error mirror is still consumed by non-migrated codegen modules and is better migrated with the rest of that slice. |
| `src/codegen/layout/class.dt` | Class layout helpers are coupled to class/type mirror structures that are not fully canonicalized yet. |
| `src/codegen/layout/vtable.dt` | Vtable layout still depends on current interface/class mirror contracts. |
| `src/codegen/types/descriptor.dt` | Descriptor helpers are tied to the non-migrated codegen/type slices. |
| `src/mono/collector.dt` | Monomorphization collector still mirrors legacy typed item and type annotation flow. |
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

What is still not true:

- the self-host mirror is **not yet** at full semantic parity with the Rust frontend for canonical syntax
- `--strict-syntax` still cannot be applied cleanly to the entire `src/` tree
- the remaining debt is concentrated in the self-host parser/typechecker/codegen/mono implementation slices

In practice, the repository is now in an intermediate state:

- the Rust frontend/tooling path is authoritative for canonical syntax
- the self-host source tree has been partially canonicalized where safe
- the remaining migration work is no longer mostly mechanical; it requires coordinated mirror updates across AST, parser, typechecker, and backend layers

## Verification Run In This Pass

- `cargo test -p draton-parser --test items -p draton-typeck --test errors`
- `cargo run -p drat -- build --strict-syntax tests/programs/gc/stress_linked_list.dt -o /tmp/draton_gc_linked_list_strict`
- `/tmp/draton_gc_linked_list_strict` returned exit code `50` as expected

## Recommended Next Steps

1. Migrate the self-host AST/item and typed-AST mirrors together.
2. Migrate `src/parser/parse/*` as one coordinated slice rather than file-by-file.
3. Migrate `src/typeck/infer/*` and `src/typeck/env/*` together so function-scope and interface-scope `@type` semantics are mirrored end-to-end.
4. Once those slices are aligned, finish codegen and mono migration.
