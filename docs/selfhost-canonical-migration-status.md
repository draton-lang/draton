# Self-Host Canonical Migration Status

This document tracks the canonical-syntax migration state of the self-host mirror under `src/`.

## Scope

Audited self-host areas:

- `src/ast/`
- `src/parser/`
- `src/typeck/`
- `src/codegen/`
- `src/lexer/`
- `src/mono/`

Canonical syntax targeted:

- `let x = ...`
- explicit `return`
- `import { item } from module.path`
- `@type { name: Type }` at file, class, layer, interface, and function scope

Deprecated syntax targeted:

- `let x: T = value`
- `fn f(a: T)`
- `fn f(...) -> T`

## Migrated In This Pass

The following remaining mechanical self-host files were canonicalized safely in this pass:

- `src/parser/parse/item.dt`
- `src/parser/parse/stmt.dt`
- `src/mono/collector.dt`
- `src/parser/parser.dt`

Migration techniques used:

- converted file-scope `@type` contracts from legacy `fn ...` members to binding form `name: (...) -> ...`
- removed inline parameter and return annotations from executable function definitions
- moved typed local bindings and typed empty-array initializers into function-scope `@type`
- moved class field annotations into class-scope `@type`
- kept parser, typechecker, and monomorphization behavior unchanged

## Previously Migrated Core

The self-host core had already been canonicalized before this pass across:

- core AST support modules
- parser expression/type helpers
- type environment, exhaustiveness, substitution, and unification helpers
- expression and statement inference helpers
- backend entry, emit/layout, and closure/helper codegen slices
- major lexer and support utilities

That earlier work also resolved the semantic parity gap for canonical contracts:

- function-scope `@type`
- interface-scope `@type`
- canonical contract flow through the self-host frontend mirror
- canonical contract flow through the self-host backend entry and emit paths

## Remaining Blocked Files

The remaining unmigrated self-host files are now explicit and non-executable.

| File | Blocker | Classification |
| --- | --- | --- |
| `src/ast/dump.dt` | Very large printer module with broad low-value mechanical churn; migrating it now would add review noise without affecting parser/typechecker/codegen behavior. | Deferred cleanup |
| `src/typeck/dump.dt` | Similar large pretty-printer module; canonicalization is straightforward but noisy and low priority compared with executable self-host paths. | Deferred cleanup |

No remaining `src/` blocker in this audit is a frontend/backend semantic-parity gap or an executable bootstrap issue.

## Final Executable Blocker Resolved

`src/typeck/infer/item.dt` was the last executable self-host file left in compatibility form.

Root cause of the earlier bootstrap regression:

- the previous direct rewrite removed inline helper-function signatures without restoring equivalent file-scope canonical contracts
- two internal helpers, `predeclare_fn_scheme` and `predeclare_class`, were missing from the file-level contract block
- once those helper schemes disappeared, bootstrap lost stable type information in the self-host inference pass and regressed

Safe fix implemented:

- converted the file-level `@type` block to canonical binding form
- added canonical file-scope contracts for the previously missing helpers
- moved typed local bindings to function-scope `@type`
- removed deprecated inline parameter and return syntax from executable function definitions

Current result:

- `src/typeck/infer/item.dt` now builds cleanly in canonical form
- self-host bootstrap passes again
- compatibility warnings from that file are gone
- the strict self-host CI exclusion list no longer includes it

## Files Completed Across The Final Phase

The final-phase blocker set has now been reduced as follows:

- completed semantic/frontend parity:
  - `src/ast/item.dt`
  - `src/parser/parse/expr.dt`
  - `src/parser/parse/item.dt`
  - `src/parser/parse/stmt.dt`
  - `src/typeck/infer/expr.dt`
  - `src/typeck/infer/stmt.dt`
  - `src/mono/collector.dt`
- completed backend parity and canonicalization:
  - `src/codegen/codegen.dt`
  - `src/codegen/emit/expr.dt`
  - `src/codegen/emit/item.dt`
  - `src/codegen/emit/stmt.dt`
  - `src/codegen/layout/class.dt`
  - `src/codegen/layout/vtable.dt`
  - `src/codegen/closure/capture.dt`
  - `src/codegen/closure/emit.dt`
  - `src/codegen/closure/env.dt`

## Current Readiness

The self-host mirror is now effectively at canonical syntax parity for executable/compiler paths:

- parser, typechecker, monomorphization, and backend/codegen slices are canonicalized where safe
- all executable self-host files are now canonicalized where safe
- the remaining debt is confined to two non-executable dump/printer modules
- a focused strict-canonical self-host CI subset is now practical

Practical strict-canonical subset:

- canonical compile fixtures
- selected GC / lambda / interface strict builds

What is not yet true:

- the entire `src/` tree is not fully strict-clean because two deferred dump/printer modules still rely on compatibility-form syntax
- the self-host bootstrap path is still tracked separately because `drat build src/main.dt` can hit `LLVM ERROR: unknown special variable`
- full-tree strict self-host CI should wait until `src/ast/dump.dt` and `src/typeck/dump.dt` are migrated, and until the separate bootstrap-path LLVM blocker is resolved or intentionally retired

## Strict-Canonical CI Subset

The repository now enforces a focused self-host strict-canonical subset in CI.

Checker:

- `python3 tools/check_selfhost_strict_subset.py`

What it does:

- scans the migrated self-host mirror under `src/`
- fails if any non-excluded file reintroduces deprecated inline type syntax
- fails if one of the tracked exclusions no longer needs to be excluded, so the list stays reviewable

Intentionally excluded files:

- `src/ast/dump.dt`
- `src/typeck/dump.dt`

This subset is intentional. It gives regression coverage for the migrated self-host tree without claiming that full-tree strict self-host support is complete or that the current bootstrap path is stable enough to be a hard CI gate.

## CI Readiness

A focused strict-canonical self-host CI subset is now practical and enabled:

- parser/typecheck regression tests cover the Rust frontend/tooling path
- `tools/check_selfhost_strict_subset.py` guards the migrated `src/` subset against compatibility-form regressions
- CI also runs one representative strict canonical fixture build

What would still be required for full-tree strict self-host CI:

1. canonicalize or intentionally retire `src/ast/dump.dt`
2. canonicalize or intentionally retire `src/typeck/dump.dt`
3. resolve the separate self-host bootstrap-path blocker currently surfacing as `LLVM ERROR: unknown special variable`

## Final Readiness

executable/compiler-path self-host canonical migration is complete.

Current repository state:

- the migrated self-host compiler path is covered by strict-canonical subset CI
- the only remaining exclusions are `src/ast/dump.dt` and `src/typeck/dump.dt`
- those files are deferred printer cleanup, not executable compiler blockers

That means contributors can now treat canonical syntax as the normal rule for executable self-host code. Full-tree strict self-host CI would still require canonicalizing or intentionally retiring the two remaining dump modules and resolving the separate bootstrap-path LLVM blocker.

## Verification Run

Historical focused verification from earlier canonical passes included:

- `cargo test -p draton-parser --test items -p draton-typeck --test errors`
- strict canonical builds for interface, lambda, generic-class, and GC fixtures
- repeated self-host-facing `cargo run -p drat -- build src/main.dt ...` bootstrap checks

Focused verification for this final mechanical pass:

- `cargo test -p draton-parser --test items -p draton-typeck --test errors`
- `cargo run -p drat -- build --strict-syntax tests/programs/compile/52_lambda_nested.dt -o /tmp/draton_mech_final_strict`
- `/tmp/draton_mech_final_strict`
- `cargo run -p drat -- build src/main.dt -o /tmp/draton_selfhost_mech_final`

Expected current behavior during bootstrap:

- self-host bootstrap remains CPU-bound and may be slow
- warning output now comes only from the deferred dump/printer modules
- the build can still hit the tracked LLVM-side blocker `LLVM ERROR: unknown special variable`
- this is not a deadlock; the main remaining cost is normal bootstrap work plus residual warning volume plus the unresolved LLVM blocker above

## Recommended Next Step

Remaining cleanup order:

1. Canonicalize `src/ast/dump.dt`
2. Canonicalize `src/typeck/dump.dt`
3. Then enable full-tree strict self-host CI if desired
