# Draton GC Benchmarks - 2026-03-18

This report publishes the next public GC benchmark snapshot for commit
`f90dcdc`.

This pass was intentionally narrower than the earlier March 18 hasher work:

- fuse young survivor promotion into the scan loop instead of building a
  separate promotion list first
- stop pushing pointer-free objects onto the major mark stack
- keep active major-cycle allocations and `protect()` semantics safe while that
  mark-stack optimization is in place

The result is mixed, and this report keeps that visible instead of smoothing it
over.

## Published artifacts

- GitHub Actions scorecard:
  [docs/benchmarks/gc-scorecard-gha-2026-03-18-run-23230366650.json](gc-scorecard-gha-2026-03-18-run-23230366650.json)
- GitHub Actions OCaml compare:
  [docs/benchmarks/gc-ocaml-compare-gha-2026-03-18-run-23230366650.json](gc-ocaml-compare-gha-2026-03-18-run-23230366650.json)
- Previous public report:
  [docs/benchmarks/gc-results-2026-03-18-e8f4d9e.md](gc-results-2026-03-18-e8f4d9e.md)

## GitHub Actions run

- workflow: `gc-perf`
- run id: `23230366650`
- repository commit: `f90dcdc`
- result: `success`

## GitHub Actions scorecard summary

Current run:

| Scenario | `elapsed_ns` |
|---|---:|
| `young-burst` | 3525346 |
| `promotion-chain` | 15976933 |
| `barrier-churn` | 7115994 |
| `old-reuse-churn` | 27672566 |
| `large-object-burst` | 4179019 |

Delta against the previous public run `23228923625`:

| Scenario | Previous `elapsed_ns` | Current `elapsed_ns` | Change |
|---|---:|---:|---:|
| `young-burst` | 3347438 | 3525346 | 5.31% slower |
| `promotion-chain` | 18399359 | 15976933 | 13.17% faster |
| `barrier-churn` | 7276970 | 7115994 | 2.21% faster |
| `old-reuse-churn` | 31130749 | 27672566 | 11.11% faster |
| `large-object-burst` | 4062662 | 4179019 | 2.86% slower |

Interpretation:

- the promotion-heavy path improved again on the same runner
- old-generation reuse also improved materially
- young-only and large-object-only scenarios regressed slightly in this pass
- the net effect is mixed rather than a clean across-the-board win

Toolchain-facing workloads in the same run:

| Workload | Status | `build_elapsed_ns` | `run_elapsed_ns` | Extra |
|---|---|---:|---:|---|
| `gc_stress_linked_list` | `ok` | 35756516727 | 1800659 | exit code `50` |
| `selfhost_bootstrap` | `blocked` | 80490507928 | - | `LLVM ERROR: unknown special variable` |

## GitHub Actions OCaml comparison

Current run summary:

- `status`: `ok`
- `scorecard_result`: `mixed`
- `draton_wins`: `0`
- `scenario_count`: `3`
- `geometric_mean_ocaml_over_draton_median_speed_ratio`: `0.26372264511307886`

Per-scenario medians:

| Scenario | Draton median ns | OCaml median ns | `ocaml_over_draton_median_speed_ratio` | Result |
|---|---:|---:|---:|---|
| `young-burst` | 647943 | 357866 | 0.5523109285847675 | Draton slower |
| `promotion-chain` | 1706554 | 130891 | 0.07669900864549262 | Draton much slower |
| `barrier-churn` | 882131 | 381946 | 0.43298104249822306 | Draton slower |

Delta against the previous public run `23228923625`:

| Metric | Previous | Current | Change |
|---|---:|---:|---:|
| `young-burst` ratio | 0.5400071740737759 | 0.5523109285847675 | 2.28% better |
| `promotion-chain` ratio | 0.08526001286120798 | 0.07669900864549262 | 10.04% worse |
| `barrier-churn` ratio | 0.47182918553087844 | 0.43298104249822306 | 8.24% worse |
| geometric mean ratio | 0.2790250801150224 | 0.26372264511307886 | 5.48% worse |

Interpretation:

- the shared OCaml comparison is still honest and still unfavorable overall
- the scorecard-side promotion improvement did not translate into a better
  relative OCaml result on this run
- the current pass is therefore a targeted runtime improvement, not a proof
  that Draton is now closing the whole gap

## Local release spot checks

Direct release-binary spot checks on the same commit during development were:

| Scenario | Direct run `elapsed_ns` |
|---|---:|
| `young-burst` | 1702670 |
| `promotion-chain` | 2570247 |
| `barrier-churn` | 2489458 |

These local numbers were useful to validate direction while iterating, but the
GitHub Actions data above remains the authoritative same-runner comparison.

## Current honest status

- Draton improved some hot runtime paths again, especially promotion and old-gen
  reuse.
- Draton still does not beat OCaml on the shared microbench suite.
- `selfhost_bootstrap` is still blocked in the scorecard for the same LLVM
  reason as before.
