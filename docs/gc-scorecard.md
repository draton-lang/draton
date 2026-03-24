# Draton GC Scorecard

GC removed in Phase 5. This scorecard is archived.

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
- background major-worker slices drained without an explicit mutator assist
- major autotune adjustments applied after a completed major cycle
- current pending major-slice budget in the runtime control plane
- peak queued major-slice budget seen during the current telemetry window
- whether major work is currently requested at the time the snapshot is taken
- safepoint rearms when the runtime keeps an active GC cycle progressing across
  multiple polls
- major-mark barrier traces for newly linked old/large children during an active
  mark phase
- remembered-set insertions and deduplicated entries
- current young, old, and total heap usage
- old-generation reusable bytes, free-slot count, and largest reusable slot
- current large-object count
- large-object free-pool block count and cached reusable bytes
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
- the runtime now tracks an explicit major-slice budget, so safepoints and
  mutator assists consume the same queue of pending major work instead of
  relying only on a boolean request flag
- a background major worker can now drain that same budget even when the
  mutator stops allocating after the mutator or safepoint path has already
  started a major cycle
- large-object sweep now feeds a reusable large-object free pool instead of
  always returning every dead block directly to the system allocator
- major-cycle completion can autotune the old-generation threshold and trim the
  large-object free pool based on live reclaim ratios
- major-work requests now raise that queue to an adaptive target based on
  threshold pressure or the current major-GC phase backlog, instead of blindly
  adding one slice per request signal
- a major cycle that has already started must continue progressing at
  safepoints until it returns to `Idle`
- stores from already-marked old/large objects must trace newly linked
  old/large children before sweep begins
- old/large objects allocated during an active major mark are born marked so
  the current cycle cannot sweep them before the mutator publishes their
  outgoing edges
- explicit `protect()` calls that add an old/large object to the root set
  during an active major mark must mark and enqueue that object immediately
- young survivors promoted into old generation during an active major mark must
  arrive in old gen already marked and queued for tracing

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

The `major_background_slices` metric counts slices drained by the background
major worker after a major cycle is already active. Rising values here mean the
collector is making forward progress without needing another explicit mutator
assist, not that it is allowed to begin a new root-scanning cycle entirely on
its own.

The `major_autotune_adjustments` metric counts automatic tuning decisions such
as threshold changes or large free-pool trims at the end of a major cycle.

The `major_work_budget` metric is the current number of queued major slices in
that control plane. It should drop back to zero when the runtime returns to an
idle major-GC state.

The `major_work_budget_peak` metric records the highest queued major-slice
budget seen since the last telemetry reset. It is the easiest way to tell
whether a workload is merely nudging the major collector or building a real
backlog that should influence future scheduler tuning.

The `large_free_pool_count` and `large_free_bytes` metrics track reusable
large-object blocks that have been swept but kept in-process for future reuse.

The `current_gc_threshold_milli` metric snapshots the effective old-generation
threshold in thousandths, so autotuning runs can be compared without parsing the
runtime config separately.

The `safepoint_rearms` metric is also workload-dependent. It increases only
when a single slow-path poll is not enough to finish the current GC work and the
runtime deliberately re-requests another safepoint poll.

## Runtime scenarios

The runtime probe currently runs five scenarios:

- `young-burst`: short-lived small-object allocation pressure
- `promotion-chain`: survivor-heavy pressure that forces promotion
- `barrier-churn`: old-to-young pointer traffic and remembered-set activity
- `old-reuse-churn`: old-generation slot reuse and fragmentation pressure
- `large-object-burst`: repeated large-object allocation and reclamation

These scenarios are implemented in
[crates/draton-runtime/examples/gc_scorecard.rs](https://github.com/draton-lang/draton/blob/main/crates/draton-runtime/examples/gc_scorecard.rs).

## Toolchain workloads

The scorecard also runs:

- `tests/programs/gc/stress_linked_list.dt`
- `examples/hello.dt`

These workloads keep the GC program tied to the actual compiler path instead of
overfitting only synthetic benchmarks.

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

Published benchmark artifacts for the current repository state live in:

- [docs/benchmarks/gc-results-2026-03-18-f90dcdc.md](benchmarks/gc-results-2026-03-18-f90dcdc.md)
- [docs/benchmarks/gc-scorecard-gha-2026-03-18-run-23230366650.json](benchmarks/gc-scorecard-gha-2026-03-18-run-23230366650.json)
- [docs/benchmarks/gc-ocaml-compare-gha-2026-03-18-run-23230366650.json](benchmarks/gc-ocaml-compare-gha-2026-03-18-run-23230366650.json)
- [docs/benchmarks/gc-results-2026-03-18-e8f4d9e.md](benchmarks/gc-results-2026-03-18-e8f4d9e.md)
- [docs/benchmarks/gc-scorecard-gha-2026-03-18-run-23228923625.json](benchmarks/gc-scorecard-gha-2026-03-18-run-23228923625.json)
- [docs/benchmarks/gc-ocaml-compare-gha-2026-03-18-run-23228923625.json](benchmarks/gc-ocaml-compare-gha-2026-03-18-run-23228923625.json)
- [docs/benchmarks/gc-results-2026-03-17.md](benchmarks/gc-results-2026-03-17.md)
- [docs/benchmarks/gc-scorecard-local-2026-03-17-d3523f5.json](benchmarks/gc-scorecard-local-2026-03-17-d3523f5.json)
- [docs/benchmarks/gc-scorecard-gha-2026-03-17-run-23203853670.json](benchmarks/gc-scorecard-gha-2026-03-17-run-23203853670.json)
- [docs/benchmarks/gc-ocaml-compare-gha-2026-03-17-run-23203853670.json](benchmarks/gc-ocaml-compare-gha-2026-03-17-run-23203853670.json)

For a matching OCaml comparison harness on Linux with `ocamlopt` installed:

```sh
python3 tools/gc_compare_ocaml.py --out build/gc-ocaml-compare.json
```

The OCaml comparison harness now runs multiple rounds per scenario, keeps the
best single sample for detailed runtime stats, and reports median-based speed
ratios plus a geometric-mean summary across scenarios. The Draton side is built
once in `--release` mode and then executed directly, so the comparison does not
accidentally include repeated `cargo run` process and compilation overhead.
That still does not constitute a blanket victory claim, but it is strong enough
to catch obvious "Draton got slower than OCaml on our core microbenchmarks"
regressions in CI.

## Interpretation

This scorecard is a baseline, not a victory claim over OCaml.

For Draton, the first success condition is:

- better compiler/bootstrap wall time
- lower or more predictable pause behavior on compiler-style workloads
- no correctness regression

Only after those are stable should the project claim broader GC superiority.
