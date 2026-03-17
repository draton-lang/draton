# Draton GC Scorecard

This document defines the baseline scorecard used to improve the Draton garbage
collector without drifting into vague "faster GC" claims.

The scorecard is intentionally split into two groups:

1. synthetic runtime scenarios that isolate collector behavior
2. real compiler-facing workloads that reflect Draton's tooling-first goals

## Official metrics

The baseline report tracks these metrics:

- allocation throughput via elapsed time on synthetic scenarios
- minor GC cycles
- major GC cycles
- major GC slices
- full GC cycles
- bytes allocated
- bytes promoted from young to old generation
- bytes reclaimed in minor collection
- bytes reclaimed in major old-gen sweep
- bytes reclaimed in large-object sweep
- write-barrier slow-path calls
- remembered-set insertions and deduplicated entries
- current young, old, and total heap usage
- current large-object count
- current root count
- current remembered-set length
- current mark-stack length and mark-slice size
- total and max pause time for minor, major-slice, and full collection paths
- wall-clock build time for representative GC/compiler workloads

These are the metrics future GC work should improve. New GC changes should not
be merged based only on subjective speed claims.

## Runtime scenarios

The runtime probe currently runs four scenarios:

- `young-burst`: short-lived small-object allocation pressure
- `promotion-chain`: survivor-heavy pressure that forces promotion
- `barrier-churn`: old-to-young pointer traffic and remembered-set activity
- `large-object-burst`: repeated large-object allocation and reclamation

These scenarios are implemented in
[draton-runtime/examples/gc_scorecard.rs](/media/lehungquangminh/QM_SSD/draton/draton-runtime/examples/gc_scorecard.rs).

## Toolchain workloads

The scorecard also runs:

- `tests/programs/gc/stress_linked_list.dt`
- `src/main.dt` bootstrap build

These workloads keep the GC program tied to the actual compiler path instead of
overfitting only synthetic benchmarks.

When the current repository state cannot build `src/main.dt` reliably enough to
serve as a hard baseline, the scorecard records that workload as `blocked`
instead of failing the entire report. That keeps the report usable while still
making the blocker explicit.

## Running the scorecard

Use:

```sh
python3 tools/gc_scorecard.py
```

Or write a report file:

```sh
python3 tools/gc_scorecard.py --out build/gc-scorecard.json
```

The report is JSON so it can feed CI, dashboards, or manual comparison runs.

## Interpretation

This scorecard is a baseline, not a victory claim over OCaml.

For Draton, the first success condition is:

- better compiler/bootstrap wall time
- lower or more predictable pause behavior on compiler-style workloads
- no correctness regression

Only after those are stable should the project claim broader GC superiority.
