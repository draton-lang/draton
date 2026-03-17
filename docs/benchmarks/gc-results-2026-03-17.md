# Draton GC Benchmarks - 2026-03-17

This report publishes the current GC benchmark artifacts for the repository
state at commit `d3523f5`.

The goal is transparency, not a victory lap.

- Current executable/compiler-path GC scorecard runs are published as JSON.
- The latest successful GitHub Actions OCaml comparison is published as JSON.
- The self-host bootstrap workload is still reported as `blocked` and is not
  hidden.
- Draton does not currently beat OCaml on the published microbenchmarks.

## Published artifacts

- Local scorecard:
  [docs/benchmarks/gc-scorecard-local-2026-03-17-d3523f5.json](gc-scorecard-local-2026-03-17-d3523f5.json)
- GitHub Actions scorecard:
  [docs/benchmarks/gc-scorecard-gha-2026-03-17-run-23203853670.json](gc-scorecard-gha-2026-03-17-run-23203853670.json)
- GitHub Actions OCaml compare:
  [docs/benchmarks/gc-ocaml-compare-gha-2026-03-17-run-23203853670.json](gc-ocaml-compare-gha-2026-03-17-run-23203853670.json)

## Data sources

Local scorecard:

- command: `python3 tools/gc_scorecard.py --out /tmp/draton-gc-scorecard-phase15.json`
- repository commit: `d3523f5`
- environment: local developer machine

GitHub Actions scorecard and OCaml compare:

- workflow: `gc-perf`
- run id: `23203853670`
- repository commit: `d3523f5`
- environment: GitHub-hosted Ubuntu runner with LLVM 14 and `ocamlopt`

Local and GitHub Actions timings should not be compared directly to each other
because the hardware and environment are different. The OCaml comparison below
uses the GitHub Actions run only, so both Draton and OCaml were measured on the
same machine class.

## Local scorecard summary

Synthetic runtime scenarios from
[docs/benchmarks/gc-scorecard-local-2026-03-17-d3523f5.json](gc-scorecard-local-2026-03-17-d3523f5.json):

| Scenario | Iterations | `elapsed_ns` | `minor_cycles` | `major_cycles` | `major_slices` | Notes |
|---|---:|---:|---:|---:|---:|---|
| `young-burst` | 20000 | 7284754 | 7 | 2 | 6 | young alloc fast path |
| `promotion-chain` | 4000 | 85078851 | 4 | 4 | 29 | survivor promotion pressure |
| `barrier-churn` | 16000 | 22105674 | 4 | 2 | 6 | `write_barrier_slow_calls=3`, `remembered_set_entries_added=3` |
| `old-reuse-churn` | 4096 | 77205419 | 6 | 6 | 31 | old-gen reuse / coalescing |
| `large-object-burst` | 256 | 6853464 | 2 | 2 | 6 | large-object free-pool reuse |

Toolchain-facing workloads from the same local scorecard:

| Workload | Status | `build_elapsed_ns` | `run_elapsed_ns` | Extra |
|---|---|---:|---:|---|
| `gc_stress_linked_list` | `ok` | 18748022233 | 1998675 | exit code `50` |
| `selfhost_bootstrap` | `blocked` | 99588654393 | - | `LLVM ERROR: unknown special variable` |

## GitHub Actions scorecard summary

Synthetic runtime scenarios from
[docs/benchmarks/gc-scorecard-gha-2026-03-17-run-23203853670.json](gc-scorecard-gha-2026-03-17-run-23203853670.json):

| Scenario | Iterations | `elapsed_ns` | `minor_cycles` | `major_cycles` | `major_slices` | Notes |
|---|---:|---:|---:|---:|---:|---|
| `young-burst` | 20000 | 3387232 | 7 | 2 | 6 | `current_gc_threshold_milli=740` |
| `promotion-chain` | 4000 | 28668218 | 4 | 4 | 23 | `major_background_slices=1` |
| `barrier-churn` | 16000 | 9783746 | 4 | 2 | 6 | `write_barrier_slow_calls=3`, `remembered_set_entries_added=3` |
| `old-reuse-churn` | 4096 | 47790843 | 6 | 6 | 29 | old-gen free-run reuse |
| `large-object-burst` | 256 | 4402338 | 2 | 3 | 9 | `large_free_pool_count=256` |

Toolchain-facing workloads from the same GitHub Actions scorecard:

| Workload | Status | `build_elapsed_ns` | `run_elapsed_ns` | Extra |
|---|---|---:|---:|---|
| `gc_stress_linked_list` | `ok` | 17366215884 | 1797229 | exit code `50` |
| `selfhost_bootstrap` | `blocked` | 79215572831 | - | `LLVM ERROR: unknown special variable` |

## GitHub Actions OCaml comparison

Source:
[docs/benchmarks/gc-ocaml-compare-gha-2026-03-17-run-23203853670.json](gc-ocaml-compare-gha-2026-03-17-run-23203853670.json)

Top-level comparison result:

- `status`: `ok`
- `scorecard_result`: `mixed`
- `draton_wins`: `0`
- `scenario_count`: `3`
- `geometric_mean_ocaml_over_draton_median_speed_ratio`: `0.21003585197381644`

Interpretation:

- A ratio below `1.0` means OCaml's median time was lower than Draton's median
  time.
- The current published result means Draton is still slower on all three OCaml
  microbench scenarios in this run.

Per-scenario median results:

| Scenario | Draton median ns | OCaml median ns | `ocaml_over_draton_median_speed_ratio` | Result |
|---|---:|---:|---:|---|
| `young-burst` | 722904 | 375032 | 0.5187853435587575 | Draton slower |
| `promotion-chain` | 2751023 | 132083 | 0.04801232123468251 | Draton much slower |
| `barrier-churn` | 1061994 | 395059 | 0.3719973935822613 | Draton slower |

## What changed before these numbers

These results include the recent GC/runtime/codegen changes already on `main`:

- skipped null write barriers in generated code
- reduced young fast-path near-full bookkeeping
- skipped shadow-stack and old-field fixups when minor GC had no young
  forwarding map
- deduplicated consecutive remembered-set parent inserts

Those changes improved the benchmark path enough to produce cleaner numbers, but
they did not yet make Draton faster than OCaml on the shared microbench set.

## Current blockers and honesty notes

- `selfhost_bootstrap` remains `blocked` in the published scorecards because the
  current self-host path still trips `LLVM ERROR: unknown special variable`.
- The published OCaml compare is not blocked anymore; it is running and
  producing real numbers.
- The repository is intentionally publishing both the local scorecard and the
  GitHub Actions compare output, including unfavorable results.

## Reproduction

Local scorecard:

```sh
python3 tools/gc_scorecard.py --out build/gc-scorecard.json
```

OCaml comparison on a machine with `ocamlopt` available:

```sh
python3 tools/gc_compare_ocaml.py --out build/gc-ocaml-compare.json
```

GitHub Actions:

```sh
gh workflow run gc-perf.yml --ref main
```
