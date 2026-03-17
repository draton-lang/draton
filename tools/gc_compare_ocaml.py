#!/usr/bin/env python3
"""Compare Draton runtime GC microbenchmarks against equivalent OCaml workloads."""

from __future__ import annotations

import argparse
import json
import math
import shutil
import statistics
import subprocess
import tempfile
import textwrap
import time
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent

OCAML_WORKLOADS: dict[str, str] = {
    "young-burst": """
        let () =
          let iterations = int_of_string Sys.argv.(1) in
          let started = Unix.gettimeofday () in
          for i = 0 to iterations - 1 do
            ignore (Array.make 8 i)
          done;
          let elapsed_ns = int_of_float ((Unix.gettimeofday () -. started) *. 1_000_000_000.) in
          print_endline (Printf.sprintf {|{"scenario":"young-burst","elapsed_ns":%d}|} elapsed_ns)
    """,
    "promotion-chain": """
        type node = { mutable next: node option; payload: int array }
        let () =
          let iterations = int_of_string Sys.argv.(1) in
          let started = Unix.gettimeofday () in
          let rec build n acc =
            if n = 0 then acc
            else build (n - 1) (Some { next = acc; payload = Array.make 8 n })
          in
          let root = build iterations None in
          Gc.full_major ();
          ignore root;
          let elapsed_ns = int_of_float ((Unix.gettimeofday () -. started) *. 1_000_000_000.) in
          print_endline (Printf.sprintf {|{"scenario":"promotion-chain","elapsed_ns":%d}|} elapsed_ns)
    """,
    "barrier-churn": """
        type child = { payload: int array }
        type parent = { mutable child: child option }
        let () =
          let iterations = int_of_string Sys.argv.(1) in
          let holder = { child = None } in
          let started = Unix.gettimeofday () in
          for i = 0 to iterations - 1 do
            holder.child <- Some { payload = Array.make 8 i }
          done;
          Gc.full_major ();
          let elapsed_ns = int_of_float ((Unix.gettimeofday () -. started) *. 1_000_000_000.) in
          print_endline (Printf.sprintf {|{"scenario":"barrier-churn","elapsed_ns":%d}|} elapsed_ns)
    """,
}


def run(args: list[str], *, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    started = time.perf_counter_ns()
    completed = subprocess.run(
        args,
        cwd=str(cwd or REPO_ROOT),
        text=True,
        capture_output=True,
        check=False,
    )
    completed.elapsed_ns = time.perf_counter_ns() - started  # type: ignore[attr-defined]
    return completed


def draton_runtime_stats(rounds: int) -> dict[str, dict[str, object]]:
    stats: dict[str, dict[str, object]] = {}
    iterations = {
        "young-burst": 20_000,
        "promotion-chain": 4_000,
        "barrier-churn": 16_000,
    }
    for scenario, count in iterations.items():
        samples: list[dict[str, object]] = []
        for _ in range(rounds):
            completed = run(
                [
                    "cargo",
                    "run",
                    "-q",
                    "-p",
                    "draton-runtime",
                    "--example",
                    "gc_scorecard",
                    "--",
                    scenario,
                    str(count),
                ]
            )
            if completed.returncode != 0:
                raise SystemExit(
                    f"draton runtime scenario failed: {scenario}\nstdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
                )
            samples.append(json.loads(completed.stdout))
        elapsed_samples = [int(sample["elapsed_ns"]) for sample in samples]
        best = min(samples, key=lambda sample: int(sample["elapsed_ns"]))
        best["elapsed_samples_ns"] = elapsed_samples
        best["median_elapsed_ns"] = int(statistics.median(elapsed_samples))
        stats[scenario] = best
    return stats


def ocaml_runtime_stats(workdir: Path, rounds: int) -> dict[str, dict[str, object]]:
    if shutil.which("ocamlopt") is None:
        return {
            "status": "blocked",
            "reason": "ocamlopt is not available in PATH",
        }

    results: dict[str, dict[str, object]] = {}
    iterations = {
        "young-burst": 20_000,
        "promotion-chain": 4_000,
        "barrier-churn": 16_000,
    }
    for scenario, source in OCAML_WORKLOADS.items():
        module_name = scenario.replace("-", "_")
        ml = workdir / f"{module_name}.ml"
        binary = workdir / scenario
        ml.write_text(textwrap.dedent(source).strip() + "\n", encoding="utf-8")
        build = run(["ocamlopt", "unix.cmxa", "-o", str(binary), str(ml)], cwd=workdir)
        if build.returncode != 0:
            return {
                "status": "blocked",
                "reason": f"failed to compile OCaml workload {scenario}",
                "stderr": build.stderr.strip(),
            }
        samples: list[dict[str, object]] = []
        for _ in range(rounds):
            executed = run([str(binary), str(iterations[scenario])], cwd=workdir)
            if executed.returncode != 0:
                return {
                    "status": "blocked",
                    "reason": f"failed to run OCaml workload {scenario}",
                    "stderr": executed.stderr.strip(),
                }
            samples.append(json.loads(executed.stdout))
        elapsed_samples = [int(sample["elapsed_ns"]) for sample in samples]
        best = min(samples, key=lambda sample: int(sample["elapsed_ns"]))
        best["elapsed_samples_ns"] = elapsed_samples
        best["median_elapsed_ns"] = int(statistics.median(elapsed_samples))
        results[scenario] = best
    return results


def compare(draton: dict[str, dict[str, object]], ocaml: dict[str, dict[str, object]]) -> dict[str, object]:
    if ocaml.get("status") == "blocked":
        return {"status": "blocked", "reason": ocaml["reason"], "details": ocaml}

    workloads = []
    wins = 0
    median_ratios: list[float] = []
    for scenario, draton_payload in draton.items():
        ocaml_payload = ocaml[scenario]
        draton_elapsed = int(draton_payload["elapsed_ns"])
        ocaml_elapsed = int(ocaml_payload["elapsed_ns"])
        ratio = ocaml_elapsed / draton_elapsed if draton_elapsed else 0.0
        draton_median = int(draton_payload.get("median_elapsed_ns", draton_elapsed))
        ocaml_median = int(ocaml_payload.get("median_elapsed_ns", ocaml_elapsed))
        median_ratio = ocaml_median / draton_median if draton_median else 0.0
        if ratio > 1.0:
            wins += 1
        if median_ratio > 0.0:
            median_ratios.append(median_ratio)
        workloads.append(
            {
                "scenario": scenario,
                "draton_elapsed_ns": draton_elapsed,
                "draton_median_elapsed_ns": draton_median,
                "ocaml_elapsed_ns": ocaml_elapsed,
                "ocaml_median_elapsed_ns": ocaml_median,
                "ocaml_over_draton_speed_ratio": ratio,
                "ocaml_over_draton_median_speed_ratio": median_ratio,
                "draton_major_slices": draton_payload["stats"]["major_slices"],
                "draton_major_background_slices": draton_payload["stats"].get(
                    "major_background_slices", 0
                ),
                "draton_current_gc_threshold_milli": draton_payload["stats"].get(
                    "current_gc_threshold_milli", 0
                ),
            }
        )
    geometric_mean_ratio = (
        math.exp(sum(math.log(ratio) for ratio in median_ratios) / len(median_ratios))
        if median_ratios
        else 0.0
    )
    return {
        "status": "ok",
        "draton_wins": wins,
        "scenario_count": len(workloads),
        "geometric_mean_ocaml_over_draton_median_speed_ratio": geometric_mean_ratio,
        "scorecard_result": "draton-faster"
        if wins == len(workloads) and geometric_mean_ratio > 1.0
        else "mixed",
        "workloads": workloads,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=None, help="Optional JSON output path")
    parser.add_argument(
        "--rounds",
        type=int,
        default=5,
        help="Benchmark rounds per scenario before computing medians",
    )
    args = parser.parse_args()

    draton = draton_runtime_stats(args.rounds)
    with tempfile.TemporaryDirectory(prefix="draton-ocaml-compare-") as temp_dir:
        ocaml = ocaml_runtime_stats(Path(temp_dir), args.rounds)
    report = {
        "generated_at_epoch_ns": time.time_ns(),
        "rounds": args.rounds,
        "draton": draton,
        "ocaml": ocaml,
        "comparison": compare(draton, ocaml),
    }
    encoded = json.dumps(report, indent=2, sort_keys=True)
    if args.out is not None:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(encoded + "\n", encoding="utf-8")
        print(args.out)
    else:
        print(encoded)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
