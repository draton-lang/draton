# Draton Self-Host Checklist

This checklist tracks the transition from the current Rust-authoritative repository to a Draton-first toolchain and the long-term goal of a repository that contains Draton plus assembly/bootstrap glue only.

The checklist is intentionally operational:

- every item must be either verifiable now or blocked by a named technical reason
- every phase has explicit exit gates
- every blocker has a reproduction command
- every completed tranche should update this file and land as its own commit

## Status legend

- `[x]` done
- `[-]` in progress
- `[ ]` not started
- `[!]` blocked by a known issue

## How To Use This Checklist

Use this file as the single execution board for self-host work.

- update `Current snapshot` after every meaningful tranche
- do not mark an item done unless its verification command has been rerun
- if an item regresses, change it back from `[x]` to `[!]` or `[-]`
- when a blocker is narrowed, replace vague text with the smallest confirmed failing scope
- when a phase gate passes, record the commit that first made it pass

## Progress Ledger

These are the committed tranches already landed during the current self-host push.

- `[x]` `ffa5374` `fix: improve self-host bootstrap diagnostics and linux codegen path`
  - `tools/verify_stage2.py` now reports signal-based failures
  - self-host Linux link path no longer hardcodes Windows-only libraries
  - self-host backend emits a native `main(argc, argv)` wrapper around `draton_user_main`
  - self-host textual LLVM uses real newlines and `double` / `float`
- `[x]` `a0a4775` `docs: add self-host progress checklist`
  - initial self-host roadmap and gate structure added
- `[x]` `e1cb534` `docs: track current self-host blockers`
  - focused blocker harness added at `tools/repro_selfhost_blockers.py`
  - parser crash narrowed to `header + main()` extracted from `src/main.dt`

## Current Snapshot

Last refreshed: `2026-03-20`

### What is currently true

- `[x]` Self-host compiler mirror exists under `src/`
- `[x]` Strict canonical self-host syntax check exists
- `[x]` Rust stage0 can build `src/main.dt` into a stage1 self-host binary
- `[x]` Stage2 verification now reports crash signals explicitly instead of failing silently
- `[x]` A focused blocker harness exists at `tools/repro_selfhost_blockers.py`
- `[x]` Self-host Linux link path no longer hardcodes Windows-only libraries
- `[x]` Self-host backend now emits a `main(argc, argv)` wrapper around `draton_user_main`
- `[x]` Self-host textual LLVM backend now emits real newlines and uses `double` / `float` instead of `f64` / `f32`
- `[x]` Self-host stage1 now builds `examples/hello.dt` successfully on Linux
- `[x]` Self-host-built `hello` binary now runs and prints `hello, draton!`
- `[x]` A checked-in parser repro fixture exists at `tests/programs/selfhost/parser_header_plus_main.dt`
- `[!]` Stage1 `check src/main.dt` still crashes with `SIGSEGV`
- `[!]` Stage1 `ast-dump src/main.dt` still crashes with `SIGSEGV`
- `[!]` Stage1 `ast-dump` on `tests/programs/selfhost/parser_header_plus_main.dt` still crashes with `SIGSEGV`

### Current blocker matrix

| Workstream | Repro command | Current result | Notes |
| --- | --- | --- | --- |
| Parser self-check | `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1` | `check-src-main -> -11` | Current crash class is `SIGSEGV` |
| Parser AST dump | `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1` | `ast-dump-src-main -> -11` | Same failure class as self-check |
| Reduced parser repro | `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1` | `ast-dump-header-plus-main -> -11` | Checked-in fixture: `tests/programs/selfhost/parser_header_plus_main.dt` |
| Linux hello fixture | `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1` | `build-hello -> 0` | String IR and print runtime blockers are cleared |

### Current baseline commands

Run these before and after each tranche.

- `[x]` `python3 tools/check_selfhost_strict_subset.py`
- `[x]` `cargo run -p drat -- build src/main.dt -o /tmp/draton_s1`
- `[x]` `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1`
- `[x]` `python3 -u tools/verify_stage2.py`

## Phase S0: Bootstrap Truth And Gates

Goal: remove ambiguity about what "self-host" means in this repository and make every bootstrap claim reproducible.

### S0.A Definitions

- `[x]` Split progress into:
  - syntax parity
  - semantic parity
  - bootstrap parity
  - runtime parity
  - backend parity
  - toolchain parity
- `[x]` Treat Rust as authoritative until S2 gate passes
- `[x]` Treat self-host under `src/` as the bootstrap target and parity mirror

### S0.B Verification harnesses

- `[x]` `tools/verify_stage2.py` prints signal-based failures
- `[x]` `tools/verify_stage2.py` runs a preflight stage1 `check src/main.dt`
- `[x]` `tools/repro_selfhost_blockers.py` exists for focused repros
- `[ ]` Add a stage3 verification path
- `[ ]` Add a one-shot readiness command that runs the agreed baseline suite
- `[ ]` Write baseline timings to a checked-in results file

### S0.C Documentation alignment

- `[ ]` Update `README` to stop implying more self-host readiness than currently proven
- `[ ]` Update `docs/selfhost-canonical-migration-status.md` to match current blocker list
- `[ ]` Update `docs/gc-scorecard.md` to reflect current bootstrap blockers and not older ones
- `[ ]` Link this checklist from the main self-host status docs

### S0 Exit Gate

- `[x]` `python3 tools/check_selfhost_strict_subset.py`
- `[x]` `python3 -u tools/verify_stage2.py` fails with a concrete blocker instead of a blank failure
- `[ ]` Stage3 verification path exists
- `[ ]` Public self-host status docs agree with the harness output

## Phase S1: Self-Host Compiler Bootstrap Stability

Goal: make the stage1 self-host binary capable of checking and rebuilding the compiler without crashing.

### S1.A Parser and frontend crash elimination

Objective: remove the `SIGSEGV` in the self-host frontend before stage2 bootstrap.

- `[x]` Confirm crash exists in both `check src/main.dt` and `ast-dump src/main.dt`
- `[x]` Narrow crash below the full compiler source
- `[x]` Confirm `header only` from `src/main.dt` parses successfully
- `[x]` Confirm `header + main()` from `src/main.dt` is sufficient to crash
- `[x]` Check in a parser regression fixture derived from the current repro
- `[ ]` Make the minimal fixture fail under an automated self-host parser test
- `[ ]` Identify whether the root cause is:
  - parser synchronization bug
  - token lifetime / rooting bug
  - AST node lifetime / rooting bug
  - another frontend memory-safety issue
- `[ ]` Fix the crash in the smallest affected parser or frontend surface
- `[ ]` Rerun the reduced fixture until it exits `0`
- `[ ]` Rerun `ast-dump src/main.dt` until it exits `0`
- `[ ]` Rerun `check src/main.dt` until it exits `0`
- `[ ]` Rerun `type-dump src/main.dt` until it exits `0`

#### S1.A Verification commands

- `[x]` `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1`
- `[x]` `/tmp/draton_s1 ast-dump tests/programs/selfhost/parser_header_plus_main.dt`
- `[ ]` `/tmp/draton_s1 ast-dump src/main.dt`
- `[ ]` `/tmp/draton_s1 check src/main.dt`
- `[ ]` `/tmp/draton_s1 type-dump src/main.dt`

#### S1.A Artifact targets

- `[x]` checked-in parser repro fixture
- `[ ]` regression test path for that fixture
- `[ ]` notes in this file naming the exact root cause once confirmed

### S1.B Self-host textual LLVM correctness

Objective: make stage1-generated textual LLVM valid enough to compile and run basic programs on Linux.

- `[x]` Use LLVM textual float types `double` and `float`
- `[x]` Emit real line breaks in generated `.ll` files
- `[x]` Emit a native `main(argc, argv)` wrapper for top-level Draton `main`
- `[x]` Remove the Linux build path dependence on Windows-only libraries
- `[x]` Fix string literal global escaping so LLVM accepts emitted constants
- `[x]` Fix string literal length accounting so constant sizes match actual bytes
- `[x]` Lower `print` / `println` to runtime symbols in self-host direct-call dispatch
- `[x]` Emit self-host LLVM fallback definitions for `draton_print` / `draton_println`
- `[x]` Verify `examples/hello.dt` builds via stage1
- `[x]` Verify `examples/hello.dt` runs via stage1
- `[ ]` Verify at least one arithmetic fixture builds and runs via stage1
- `[ ]` Verify at least one branch/control-flow fixture builds and runs via stage1
- `[ ]` Verify at least one class/layer fixture builds via stage1

#### S1.B Verification commands

- `[x]` `python3 tools/repro_selfhost_blockers.py --stage1 /tmp/draton_s1`
- `[x]` `DRATON_RUNTIME_LIB=... /tmp/draton_s1 build examples/hello.dt -o /tmp/selfhost_hello`
- `[x]` `/tmp/selfhost_hello`
- `[ ]` stage1 build and run commands for arithmetic and control-flow fixtures

#### S1.B Artifact targets

- `[x]` checked-in notes of the string-literal IR root cause
- `[ ]` at least three passing Linux stage1 executable fixtures

### S1.C Bootstrap ladder

Objective: move from "Rust can build stage1" to "Draton can rebuild itself repeatedly".

- `[x]` Rust stage0 builds stage1
- `[!]` Stage1 self-check on `src/main.dt` passes
- `[ ]` Stage1 builds stage2
- `[ ]` Stage2 self-check on `src/main.dt` passes
- `[ ]` Stage2 builds stage3
- `[ ]` Stage3 self-check on `src/main.dt` passes
- `[ ]` Stage2 and stage3 exhibit matching CLI behavior on the bootstrap corpus
- `[ ]` `tools/verify_stage2.py` passes end to end
- `[ ]` Stage3 verification command exists and passes end to end

#### S1.C Verification commands

- `[x]` `cargo run -p drat -- build src/main.dt -o /tmp/draton_s1`
- `[ ]` `/tmp/draton_s1 check src/main.dt`
- `[ ]` `/tmp/draton_s1 build src/main.dt -o /tmp/draton_s2`
- `[ ]` `/tmp/draton_s2 check src/main.dt`
- `[ ]` `/tmp/draton_s2 build src/main.dt -o /tmp/draton_s3`
- `[ ]` `/tmp/draton_s3 check src/main.dt`
- `[ ]` `python3 -u tools/verify_stage2.py`

### S1 Exit Gate

- `[ ]` Stage1 no longer crashes on `src/main.dt`
- `[ ]` Stage1 builds and runs `examples/hello.dt`
- `[ ]` Stage1 builds stage2
- `[ ]` Stage2 builds stage3
- `[ ]` Stage2 and stage3 agree on the bootstrap corpus

## Phase S2: Promote Draton Compiler-Core To Primary

Goal: move compiler-core authority from Rust to Draton only after bootstrap stability is real.

### S2.A Parity discipline

- `[ ]` Define the selected parity corpus for frontend behavior
- `[ ]` Compare Rust stage0 vs Draton stage1 on:
  - `ast-dump`
  - `type-dump`
  - `check`
  - executable fixtures
- `[ ]` Record known mismatches explicitly instead of leaving them implicit
- `[ ]` Add anti-drift checks so new semantic changes do not land only in Rust

### S2.B Interface cleanup

- `[ ]` Split compiler-core surfaces in Draton into explicit layers:
  - lex
  - parse
  - check
  - mono
  - emit
- `[ ]` Separate host-facing services from compiler-core logic
- `[ ]` Mark which surfaces are allowed to depend on runtime or host ABI

### S2.C Source-of-truth transition

- `[ ]` Document the exact acceptance conditions for promoting `src/` to primary compiler-core
- `[ ]` Switch status docs from "mirror" to "primary" only after S2 gate passes
- `[ ]` Re-scope Rust crates as bootstrap/parity references

### S2 Exit Gate

- `[ ]` Draton stage1 and Rust stage0 agree on the selected parity corpus
- `[ ]` Stage2 and stage3 remain stable across repeated bootstrap runs
- `[ ]` No new language semantic change lands only in Rust

## Phase S3: Runtime And Host Surface Extraction

Goal: stop depending on the Rust runtime crate for the normal bootstrap path.

### S3.A Linux x86_64 host ABI minimum

- `[ ]` file read
- `[ ]` file write
- `[ ]` process exec
- `[ ]` argv
- `[ ]` env
- `[ ]` stdout
- `[ ]` stderr
- `[ ]` wall-clock time
- `[ ]` monotonic time
- `[ ]` heap allocation primitive
- `[ ]` bootstrap host ABI document

### S3.B Bootstrap-minimal runtime

- `[ ]` startup / shutdown glue
- `[ ]` string primitives needed by compiler-core
- `[ ]` array primitives needed by compiler-core
- `[ ]` panic path
- `[ ]` allocation path sufficient for bootstrap
- `[ ]` documented bootstrap-mode GC or non-GC policy
- `[ ]` separate bootstrap-minimal runtime from full runtime ambitions

### S3.C Stdlib surface needed for bootstrap

- `[ ]` `io`
- `[ ]` `string`
- `[ ]` `os`
- `[ ]` `fs`
- `[ ]` `time`
- `[ ]` `collections`
- `[ ]` `json`
- `[ ]` `math`
- `[ ]` leave `net` deferred unless needed
- `[ ]` leave `crypto` deferred unless needed

### S3 Exit Gate

- `[ ]` Stage1 compiler-core and bootstrap toolchain run without the Rust runtime crate
- `[ ]` Required stdlib modules used by bootstrap no longer depend on Rust-backed FFI

## Phase S4: Direct Assembly Backend

Goal: replace the LLVM-text path with a native assembly path for the first supported host target.

### S4.A Backend architecture

- `[ ]` Freeze initial backend target to `linux-x86_64`
- `[ ]` Define a stable internal lowering boundary after typecheck / mono
- `[ ]` Define calling convention policy
- `[ ]` Define stack-frame policy
- `[ ]` Define data section and string/global layout policy
- `[ ]` Define external symbol ABI for runtime hooks

### S4.B Backend implementation

- `[ ]` integer arithmetic
- `[ ]` comparisons
- `[ ]` branches
- `[ ]` structured control flow
- `[ ]` function calls
- `[ ]` returns
- `[ ]` local stack slots
- `[ ]` string/object references needed for bootstrap
- `[ ]` entrypoint emission
- `[ ]` assembler invocation
- `[ ]` linker invocation

### S4.C Backend verification

- `[ ]` build and run constant/arithmetic fixtures
- `[ ]` build and run control-flow fixtures
- `[ ]` build and run string printing fixtures
- `[ ]` build and run compiler-facing subset
- `[ ]` build the self-host compiler with the assembly backend

### S4 Exit Gate

- `[ ]` Direct-asm backend builds and runs the bootstrap fixture set on Linux
- `[ ]` Direct-asm backend builds the self-host compiler itself

## Phase S5: Full Toolchain In Draton

Goal: move from self-host compiler-core to self-host day-to-day tooling.

### S5.A Core commands already present in Draton

- `[x]` `build`
- `[x]` `run`
- `[x]` `check`
- `[x]` `ast-dump`
- `[x]` `type-dump`

### S5.B Commands still to port

- `[ ]` `fmt`
- `[ ]` `lint`
- `[ ]` `task`
- `[ ]` `test`
- `[ ]` `doc`
- `[ ]` `repl`
- `[ ]` `lsp`
- `[ ]` package management commands
- `[ ]` publish / update commands

### S5.C Tooling quality gates

- `[ ]` formatter regression corpus
- `[ ]` lint corpus
- `[ ]` task runner smoke suite
- `[ ]` test command smoke suite
- `[ ]` doc generation smoke suite
- `[ ]` LSP smoke suite
- `[ ]` package workflow smoke suite

### S5 Exit Gate

- `[ ]` Draton-first toolchain covers the commands needed for normal compiler development
- `[ ]` Rust `drat` CLI can be retired or reduced to bootstrap-only compatibility

## Phase S6: Rust Retirement

Goal: reach the repository shape "Draton + assembly/bootstrap glue" without Rust in the normal path.

- `[ ]` Remove Rust as source of truth for compiler-core
- `[ ]` Remove Rust runtime crate from the normal build path
- `[ ]` Remove Rust-backed stdlib implementation from the normal build path
- `[ ]` Remove Rust CLI/tooling from the normal build path
- `[ ]` Keep only assembly/bootstrap glue that remains justified and documented
- `[ ]` Document the final bootstrap chain from released artifact to self-host rebuild

### S6 Exit Gate

- `[ ]` Repository can bootstrap, build, and run the official toolchain without Rust source code participating in the normal path
- `[ ]` Remaining non-Draton code is limited to explicit assembly/bootstrap glue

## Immediate Next Tasks

These are the tasks that should move next unless a newly discovered blocker supersedes them.

### Active tranche

- `[ ]` Fix self-host parser `SIGSEGV` on `src/main.dt`
- `[ ]` Check in a minimal parser regression fixture for the current crash
- `[ ]` Rerun `tools/verify_stage2.py` after parser/frontend crash is fixed
- `[ ]` Update this checklist immediately after the next tranche lands

### Ready after current blockers

- `[ ]` Add stage3 verification path
- `[ ]` Add a one-shot self-host readiness command
- `[ ]` Add arithmetic and control-flow Linux stage1 fixtures now that `hello.dt` passes
- `[ ]` Start a small parity corpus for Rust stage0 vs Draton stage1

## Definition Of "Good Update"

Each future checklist update should include:

- the exact command rerun
- the exact result observed
- the smallest remaining blocker if the item is still not done
- the commit that changed the state, once the change is committed
