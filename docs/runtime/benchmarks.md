---
title: Benchmark snapshots
sidebar_position: 31
---

# Benchmark snapshots

This page links benchmark artifacts that are already committed in the repository.

Draton now uses Inferred Ownership for safe code. The GC-related artifacts on this page are archived historical snapshots from before the migration, not the current runtime design.

## Scorecards

- [GC scorecard overview (archived)](../gc-scorecard.md)
- [Local snapshot: 2026-03-17 d3523f5](../benchmarks/gc-scorecard-local-2026-03-17-d3523f5.json)
- [GitHub Actions snapshot: 2026-03-17 run 23203853670](../benchmarks/gc-scorecard-gha-2026-03-17-run-23203853670.json)
- [GitHub Actions snapshot: 2026-03-18 run 23228923625](../benchmarks/gc-scorecard-gha-2026-03-18-run-23228923625.json)
- [GitHub Actions snapshot: 2026-03-18 run 23230366650](../benchmarks/gc-scorecard-gha-2026-03-18-run-23230366650.json)

## Draton versus OCaml comparison artifacts

- [2026-03-17 run 23203853670](../benchmarks/gc-ocaml-compare-gha-2026-03-17-run-23203853670.json)
- [2026-03-18 run 23228923625](../benchmarks/gc-ocaml-compare-gha-2026-03-18-run-23228923625.json)
- [2026-03-18 run 23230366650](../benchmarks/gc-ocaml-compare-gha-2026-03-18-run-23230366650.json)

## Human-readable result summaries

- [GC results: 2026-03-17 (archived)](../benchmarks/gc-results-2026-03-17.md)
- [GC results: 2026-03-18 e8f4d9e (archived)](../benchmarks/gc-results-2026-03-18-e8f4d9e.md)
- [GC results: 2026-03-18 f90dcdc (archived)](../benchmarks/gc-results-2026-03-18-f90dcdc.md)

## How to read them

These files are not marketing snapshots. They are evidence:

- use the JSON files for raw numbers and machine-readable history
- use the markdown summaries for a quick human pass
- use [gc-scorecard](../gc-scorecard.md) for the meaning of each archived GC benchmark and metric
