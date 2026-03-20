# Draton Self-Host Checklist

This checklist tracks the transition from the current Rust-authoritative repository to a Draton-first toolchain and the long-term goal of a repository that contains Draton plus assembly/bootstrap glue only.

The checklist is intentionally operational:

- every item should be either verifiable now or blocked by a named technical reason
- every phase has explicit gates
- "self-host" is split into compiler-core, bootstrap, runtime, backend, and full-toolchain milestones so progress is measurable instead of rhetorical

## Status legend

- `[x]` done
- `[-]` in progress
- `[ ]` not started
- `[!]` blocked by a known issue

## Current snapshot

- `[x]` Self-host compiler mirror exists under `src/`
- `[x]` Strict canonical self-host syntax check exists
- `[x]` Rust stage0 can build `src/main.dt` into a stage1 self-host binary
- `[x]` Stage2 verification now reports crash signals explicitly instead of failing silently
- `[x]` Self-host Linux link path no longer hardcodes Windows-only libraries
- `[x]` Self-host backend now emits a `main(argc, argv)` wrapper around `draton_user_main`
- `[x]` Self-host textual LLVM backend now emits real newlines and uses `double`/`float` instead of `f64`/`f32`
- `[!]` Stage1 `check src/main.dt` still crashes with `SIGSEGV`
- `[!]` Self-host `build examples/hello.dt` still fails in string-literal LLVM IR escaping/length accounting

## Phase S0: Bootstrap Truth And Gates

- `[x]` Define the working milestone ladder:
  - syntax parity
  - semantic parity
  - bootstrap parity
  - runtime parity
  - toolchain parity
- `[x]` Make `tools/verify_stage2.py` print signal-based failures
- `[x]` Add a preflight `stage1 check src/main.dt` gate before stage2 build
- `[ ]` Add a dedicated stage3 verification script or stage3 mode to the existing script
- `[ ]` Record the current baseline timings for stage1, stage2, stage3 in a stable results file
- `[ ]` Add a single command that summarizes self-host readiness in one place
- `[ ]` Update public docs so `README`, `docs/selfhost-canonical-migration-status.md`, and `docs/gc-scorecard.md` stop disagreeing about current self-host readiness

### S0 gate

- `[x]` `python3 tools/check_selfhost_strict_subset.py`
- `[x]` `python3 -u tools/verify_stage2.py` now fails with a concrete blocker instead of a blank failure
- `[ ]` Stage summary command committed and documented

## Phase S1: Self-Host Compiler Bootstrap Stability

### Parser and frontend crash elimination

- `[!]` Fix the stage1 parser crash on `src/main.dt`
- `[ ]` Reduce the current `SIGSEGV` to a checked-in minimal parser fixture
- `[ ]` Add a self-host parser regression test for the reduced fixture
- `[ ]` Confirm `ast-dump src/main.dt` no longer crashes
- `[ ]` Confirm `check src/main.dt` no longer crashes
- `[ ]` Confirm `type-dump src/main.dt` no longer crashes

### Self-host codegen textual IR correctness

- `[x]` Use LLVM textual float types (`double` and `float`)
- `[x]` Emit real line breaks in generated `.ll` files
- `[x]` Emit a native `main(argc, argv)` wrapper for top-level Draton `main`
- `[!]` Fix string literal global escaping so LLVM accepts generated constants
- `[ ]` Verify `examples/hello.dt` builds and runs via stage1 on Linux
- `[ ]` Verify at least one arithmetic program builds and runs via stage1 on Linux
- `[ ]` Verify at least one class/layer fixture builds via stage1 on Linux

### Stage ladder

- `[x]` Rust stage0 builds stage1
- `[!]` Stage1 self-check on `src/main.dt` passes
- `[ ]` Stage1 builds stage2
- `[ ]` Stage2 builds stage3
- `[ ]` Stage2 and stage3 produce matching behavior on the bootstrap corpus

### S1 gate

- `[ ]` `/tmp/draton_s1 check src/main.dt` exits `0`
- `[ ]` `/tmp/draton_s1 build examples/hello.dt -o ...` exits `0`
- `[ ]` `/tmp/draton_s2 --help` matches stage1
- `[ ]` `python3 -u tools/verify_stage2.py` passes all cases

## Phase S2: Promote Draton Compiler-Core To Primary

- `[ ]` Decide and document when `src/` becomes the primary compiler-core implementation
- `[ ]` Make all frontend semantic fixes land in Draton first or in Rust and Draton together
- `[ ]` Add a parity suite that compares Rust stage0 vs Draton stage1 for:
  - `ast-dump`
  - `type-dump`
  - `check`
  - selected executable fixtures
- `[ ]` Split compiler-core interfaces in Draton into explicit surfaces:
  - lex
  - parse
  - check
  - emit
- `[ ]` Mark Rust crates as `bootstrap/parity reference` instead of source of truth once the gate is met

### S2 gate

- `[ ]` Draton stage1 and Rust stage0 agree on the selected frontend corpus
- `[ ]` Stage2 and stage3 remain stable across repeated bootstrap runs
- `[ ]` No new language semantic change is merged only in Rust

## Phase S3: Runtime And Host Surface Extraction

### Host ABI minimum for Linux x86_64

- `[ ]` File read/write
- `[ ]` Process exec
- `[ ]` `argv` / `env`
- `[ ]` stdout / stderr
- `[ ]` wall-clock and monotonic time
- `[ ]` heap allocation primitive
- `[ ]` explicit bootstrap ABI document for this host layer

### Runtime minimum

- `[ ]` startup / shutdown glue
- `[ ]` string and array primitives required by compiler-core
- `[ ]` panic path
- `[ ]` memory management path sufficient for compiler bootstrap
- `[ ]` GC policy for bootstrap mode documented
- `[ ]` split bootstrap-minimal runtime from full runtime ambitions

### Stdlib surface needed for bootstrap

- `[ ]` `io`
- `[ ]` `string`
- `[ ]` `os`
- `[ ]` `fs`
- `[ ]` `time`
- `[ ]` `collections`
- `[ ]` `json`
- `[ ]` `math`
- `[ ]` `net` deferred unless required by toolchain
- `[ ]` `crypto` deferred unless required by toolchain

### S3 gate

- `[ ]` Compiler-core and stage1 toolchain run without the Rust runtime crate on Linux
- `[ ]` Required stdlib modules used by bootstrap no longer depend on Rust-backed FFI

## Phase S4: Direct Assembly Backend

### Backend architecture

- `[ ]` Freeze the initial backend target to `linux-x86_64`
- `[ ]` Define an internal post-typecheck lowering boundary for codegen
- `[ ]` Define calling convention and stack-frame policy
- `[ ]` Define data section and string/global layout policy
- `[ ]` Define external symbol ABI for runtime hooks

### Backend implementation

- `[ ]` integer arithmetic and comparisons
- `[ ]` branches and structured control flow
- `[ ]` function calls and returns
- `[ ]` local stack slots
- `[ ]` string/object references required for compiler bootstrap
- `[ ]` top-level entrypoint emission
- `[ ]` assembler invocation
- `[ ]` linker invocation

### Backend verification

- `[ ]` build and run constant/arithmetic fixtures
- `[ ]` build and run control-flow fixtures
- `[ ]` build and run string printing fixtures
- `[ ]` build and run compiler-facing subset
- `[ ]` build the self-host compiler with the assembly backend

### S4 gate

- `[ ]` Direct-asm backend builds and runs the bootstrap fixture set on Linux
- `[ ]` Direct-asm backend builds the self-host compiler itself

## Phase S5: Full Toolchain In Draton

### Core commands

- `[x]` `build`
- `[x]` `run`
- `[x]` `check`
- `[x]` `ast-dump`
- `[x]` `type-dump`

### Tooling to port

- `[ ]` `fmt`
- `[ ]` `lint`
- `[ ]` `task`
- `[ ]` `test`
- `[ ]` `doc`
- `[ ]` `repl`
- `[ ]` `lsp`
- `[ ]` package management commands
- `[ ]` publish/update commands

### Tooling quality gates

- `[ ]` formatter regression corpus
- `[ ]` lint corpus
- `[ ]` task runner smoke suite
- `[ ]` LSP smoke suite
- `[ ]` package workflow smoke suite

### S5 gate

- `[ ]` Draton-first toolchain covers the commands needed for normal compiler development
- `[ ]` Rust `drat` CLI can be retired or reduced to a bootstrap-only compatibility shell

## Phase S6: Rust Retirement

- `[ ]` Remove Rust as source of truth for compiler-core
- `[ ]` Remove Rust runtime crate from normal build path
- `[ ]` Remove Rust-backed stdlib implementation from normal build path
- `[ ]` Remove Rust CLI/tooling from normal build path
- `[ ]` Keep only assembly/bootstrap glue that is still justified and documented
- `[ ]` Document the final bootstrap chain from released artifact to self-host rebuild

### S6 gate

- `[ ]` Repository can bootstrap, build, and run the official toolchain without Rust source code participating in the normal path
- `[ ]` Remaining non-Draton code is limited to explicit assembly/bootstrap glue

## Immediate next tasks

- `[ ]` Fix self-host parser `SIGSEGV` on `src/main.dt`
- `[ ]` Add a minimal checked-in repro for that parser crash
- `[ ]` Fix self-host string-literal LLVM IR escaping so `examples/hello.dt` builds via stage1
- `[ ]` Rerun `tools/verify_stage2.py`
- `[ ]` Update this checklist after the next tranche lands
