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
- total major-work request signals
- threshold-driven major-work request signals
- active-cycle continuation request signals
- major-mutator assists when an allocation slow path, including young refills,
  helps drain pending major work
- whether major work is currently requested at the time the snapshot is taken
- safepoint rearms when the runtime keeps an active GC cycle progressing across
  multiple polls
- major-mark barrier traces for newly linked old/large children during an active
  mark phase
- remembered-set insertions and deduplicated entries
- current young, old, and total heap usage
- old-generation reusable bytes, free-slot count, and largest reusable slot
- current large-object count
- current root count
- current remembered-set length
- current mark-stack length and mark-slice size
- total and max pause time for minor, major-slice, and full collection paths
- wall-clock build time for representative GC/compiler workloads

These are the metrics future GC work should improve. New GC changes should not
be merged based only on subjective speed claims.

The runtime also exposes heap-invariant verification. Any allocator or collector
change that improves scorecard numbers but breaks verifier checks is invalid.

Current baseline assumptions:

- major GC mark and sweep both run incrementally in bounded slices
- old-generation free runs are rebuilt during sweep and coalesced across slice
  boundaries before they re-enter the allocator free lists
- major-GC scheduling uses an explicit request flag, so promotion pressure and
  active major cycles do not depend only on re-deriving threshold state at each
  individual safepoint
- allocation slow paths may assist pending major work with one bounded slice,
  including the young-refill path after a minor collection, reducing the
  runtime's dependence on a separate safepoint poll before the next chunk of
  major work can run
- a major cycle that has already started must continue progressing at
  safepoints until it returns to `Idle`
- stores from already-marked old/large objects must trace newly linked
  old/large children before sweep begins

The `major_mark_barrier_traces` metric is expected to stay at zero on many
synthetic runs. It becomes non-zero only when mutator stores overlap with an
active major mark phase.

The `major_work_requests` metric counts all explicit signals that request more
major-GC work. `major_work_threshold_requests` isolates the signals caused by
crossing the old-generation threshold, while
`major_work_continuation_requests` isolates the signals caused by an already
active major cycle asking for another slice. `major_work_requested` is the
current snapshot of that control flag and is expected to be `true` when
promotion pressure or an active cycle still wants another safepoint-driven
slice.

The `major_mutator_assists` metric counts slow-path allocations that helped run
one pending major-GC slice immediately instead of waiting for a later poll.

The `safepoint_rearms` metric is also workload-dependent. It increases only
when a single slow-path poll is not enough to finish the current GC work and the
runtime deliberately re-requests another safepoint poll.

## Runtime scenarios

The runtime probe currently runs four scenarios:

- `young-burst`: short-lived small-object allocation pressure
- `promotion-chain`: survivor-heavy pressure that forces promotion
- `barrier-churn`: old-to-young pointer traffic and remembered-set activity
- `old-reuse-churn`: old-generation slot reuse and fragmentation pressure
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
