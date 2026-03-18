# Draton GC Benchmarks - 2026-03-18

This report publishes the next public GC benchmark snapshot for commit
`e8f4d9e`.

The purpose is still transparency:

- publish the new GitHub Actions scorecard for the current head
- publish the new GitHub Actions OCaml comparison for the current head
- show the delta against the last published run instead of only showing the
  current numbers
- keep the unfavorable result visible: Draton still loses all three OCaml
  microbench scenarios in this run

## Published artifacts

- GitHub Actions scorecard:
  [docs/benchmarks/gc-scorecard-gha-2026-03-18-run-23228923625.json](gc-scorecard-gha-2026-03-18-run-23228923625.json)
- GitHub Actions OCaml compare:
  [docs/benchmarks/gc-ocaml-compare-gha-2026-03-18-run-23228923625.json](gc-ocaml-compare-gha-2026-03-18-run-23228923625.json)
- Previous public report:
  [docs/benchmarks/gc-results-2026-03-17.md](gc-results-2026-03-17.md)

## What changed in this pass

This benchmark snapshot includes one more runtime optimization pass on top of
the earlier March 17 baseline:

- internal GC maps for roots, forwarding entries, large objects, and type
  descriptors now use a dedicated fast hasher instead of the default `HashMap`
  hasher
- hot collector paths no longer clone `TypeDescriptor` values just to walk
  pointer offsets

That change targets exactly the remaining hot paths that still looked expensive:

- root/protect lookup overhead
- forwarding lookup overhead during promotion
- descriptor lookup overhead during tracing and fixup

## GitHub Actions run

- workflow: `gc-perf`
- run id: `23228923625`
- repository commit: `e8f4d9e`
- result: `success`

## GitHub Actions scorecard summary

Current run:

| Scenario | `elapsed_ns` |
|---|---:|
| `young-burst` | 3347438 |
| `promotion-chain` | 18399359 |
| `barrier-churn` | 7276970 |
| `old-reuse-churn` | 31130749 |
| `large-object-burst` | 4062662 |

Delta against the previous public run `23203853670`:

| Scenario | Previous `elapsed_ns` | Current `elapsed_ns` | Improvement |
|---|---:|---:|---:|
| `young-burst` | 3387232 | 3347438 | 1.17% faster |
| `promotion-chain` | 28668218 | 18399359 | 35.82% faster |
| `barrier-churn` | 9783746 | 7276970 | 25.62% faster |
| `old-reuse-churn` | 47790843 | 31130749 | 34.86% faster |
| `large-object-burst` | 4402338 | 4062662 | 7.71% faster |

Toolchain-facing workloads in the same run:

| Workload | Status | `build_elapsed_ns` | `run_elapsed_ns` | Extra |
|---|---|---:|---:|---|
| `gc_stress_linked_list` | `ok` | 17300264552 | 1718897 | exit code `50` |
| `selfhost_bootstrap` | `blocked` | 78657157430 | - | `LLVM ERROR: unknown special variable` |

## GitHub Actions OCaml comparison

Current run summary:

- `status`: `ok`
- `scorecard_result`: `mixed`
- `draton_wins`: `0`
- `scenario_count`: `3`
- `geometric_mean_ocaml_over_draton_median_speed_ratio`: `0.2790250801150224`

Per-scenario medians:

| Scenario | Draton median ns | OCaml median ns | `ocaml_over_draton_median_speed_ratio` | Result |
|---|---:|---:|---:|---|
| `young-burst` | 683015 | 368833 | 0.5400071740737759 | Draton slower |
| `promotion-chain` | 1898733 | 161886 | 0.08526001286120798 | Draton much slower |
| `barrier-churn` | 860536 | 406026 | 0.47182918553087844 | Draton slower |

Delta against the previous public run `23203853670`:

| Metric | Previous | Current | Change |
|---|---:|---:|---:|
| `young-burst` ratio | 0.5187853435587575 | 0.5400071740737759 | 4.09% better |
| `promotion-chain` ratio | 0.04801232123468251 | 0.08526001286120798 | 77.58% better |
| `barrier-churn` ratio | 0.3719973935822613 | 0.47182918553087844 | 26.84% better |
| geometric mean ratio | 0.21003585197381644 | 0.2790250801150224 | 32.85% better |

Interpretation:

- the new pass clearly improved Draton on all three shared OCaml scenarios
- the biggest gain is still on `promotion-chain`
- this is real progress, not a benchmark harness change
- Draton is still slower than OCaml on all three scenarios in this run

## Local spot checks

The full local scorecard still takes longer because the script also walks the
blocked self-host bootstrap path. For quick validation on the same commit, the
direct scenario spot checks on the local machine were:

| Scenario | Direct run `elapsed_ns` |
|---|---:|
| `young-burst` | 5778731 |
| `promotion-chain` | 31835046 |
| `barrier-churn` | 15307658 |

These local numbers are useful for sanity-checking the direction of change, but
the GitHub Actions OCaml compare above remains the authoritative same-machine
comparison.

## Current honest status

- Draton is faster than before on the published scorecard.
- Draton still does not beat OCaml on the shared microbench suite.
- `selfhost_bootstrap` is still blocked in the scorecard for the same LLVM
  reason as before.
