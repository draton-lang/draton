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

The remaining unmigrated self-host files are now small and explicit.

| File | Blocker | Classification |
| --- | --- | --- |
| `src/typeck/infer/item.dt` | A direct canonical rewrite was attempted in this pass, but the self-host-facing bootstrap build regressed with core inference failures. The compatibility-form source is still stable, so this file remains intentionally unmigrated until that inference gap is understood cleanly. | Mechanical, but not yet safe |
| `src/ast/dump.dt` | Very large printer module with broad low-value mechanical churn; migrating it now would add review noise without affecting parser/typechecker/codegen behavior. | Deferred cleanup |
| `src/typeck/dump.dt` | Similar large pretty-printer module; canonicalization is straightforward but noisy and low priority compared with executable self-host paths. | Deferred cleanup |

No remaining `src/` blocker in this audit is a major frontend/backend semantic-parity gap. The main unresolved executable file is a migration-safety issue in `src/typeck/infer/item.dt`.

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

The self-host mirror is now effectively at canonical syntax parity for executable compiler paths:

- parser, typechecker, monomorphization, and backend/codegen slices are canonicalized where safe
- only one executable self-host file remains in compatibility form: `src/typeck/infer/item.dt`
- the other remaining debt is confined to two non-executable dump/printer modules
- a focused strict-canonical self-host CI subset is now practical

Practical strict-canonical subset:

- canonical compile fixtures
- selected GC / lambda / interface strict builds
- self-host-facing bootstrap builds that exercise parser, mono, and backend paths, while tolerating compatibility warnings from `src/typeck/infer/item.dt` and the dump modules

What is not yet true:

- the entire `src/` tree is not fully strict-clean because `src/typeck/infer/item.dt` still relies on compatibility-form syntax
- full-tree strict self-host CI should wait until `src/typeck/infer/item.dt` and the two dump modules are migrated, or explicitly exclude them

## Strict-Canonical CI Subset

The repository now enforces a focused self-host strict-canonical subset in CI.

Checker:

- `python3 tools/check_selfhost_strict_subset.py`

What it does:

- scans the migrated self-host mirror under `src/`
- fails if any non-excluded file reintroduces deprecated inline type syntax
- fails if one of the tracked exclusions no longer needs to be excluded, so the list stays reviewable

Intentionally excluded files:

- `src/typeck/infer/item.dt`
- `src/ast/dump.dt`
- `src/typeck/dump.dt`

This subset is intentional. It gives regression coverage for the migrated self-host tree without claiming that full-tree strict self-host support is complete.

## CI Readiness

A focused strict-canonical self-host CI subset is now practical and enabled:

- parser/typecheck regression tests cover the Rust frontend/tooling path
- `tools/check_selfhost_strict_subset.py` guards the migrated `src/` subset against compatibility-form regressions
- CI also runs one representative strict canonical fixture build and one self-host-facing bootstrap build

What would still be required for full-tree strict self-host CI:

1. safely canonicalize `src/typeck/infer/item.dt`
2. canonicalize or intentionally retire `src/ast/dump.dt`
3. canonicalize or intentionally retire `src/typeck/dump.dt`

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
- warning output now comes from `src/typeck/infer/item.dt` plus the deferred dump/printer modules
- this is not a deadlock; the main remaining cost is normal bootstrap work plus residual warning volume

## Recommended Next Step

Remaining cleanup order:

1. Investigate and safely canonicalize `src/typeck/infer/item.dt`
2. Canonicalize `src/ast/dump.dt`
3. Canonicalize `src/typeck/dump.dt`
4. Then enable full-tree strict self-host CI if desired
