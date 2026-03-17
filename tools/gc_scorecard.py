#!/usr/bin/env python3
"""Run the Draton GC scorecard baseline and emit a JSON report."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
SCENARIOS = (
    ("young-burst", 20000),
    ("promotion-chain", 4000),
    ("barrier-churn", 16000),
    ("old-reuse-churn", 4096),
    ("large-object-burst", 256),
)


def run(
    args: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    started = time.perf_counter_ns()
    merged_env = os.environ.copy()
    if env is not None:
        merged_env.update(env)
    completed = subprocess.run(
        args,
        cwd=str(cwd or REPO_ROOT),
        env=merged_env,
        text=True,
        capture_output=True,
        check=False,
    )
    completed.elapsed_ns = time.perf_counter_ns() - started  # type: ignore[attr-defined]
    return completed


def run_runtime_scenarios() -> list[dict[str, object]]:
    results: list[dict[str, object]] = []
    for name, iterations in SCENARIOS:
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
                name,
                str(iterations),
            ]
        )
        if completed.returncode != 0:
            raise SystemExit(
                f"runtime scenario failed: {name}\nstdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
            )
        payload = json.loads(completed.stdout)
        payload["command_elapsed_ns"] = completed.elapsed_ns  # type: ignore[index]
        results.append(payload)
    return results


def run_toolchain_workloads(workdir: Path) -> list[dict[str, object]]:
    workloads: list[dict[str, object]] = []
    drat_env = {
        "DRATON_ALLOW_MULTIPLE_RUNTIME_DEFS": "1",
        "DRATON_DISABLE_GCROOT": "1",
    }

    linked_list_bin = workdir / "stress_linked_list"
    build_stress = run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "drat",
            "--",
            "build",
            "--strict-syntax",
            "tests/programs/gc/stress_linked_list.dt",
            "-o",
            str(linked_list_bin),
        ],
        env=drat_env,
    )
    if build_stress.returncode != 0:
        raise SystemExit(
            f"failed to build stress_linked_list.dt\nstdout:\n{build_stress.stdout}\nstderr:\n{build_stress.stderr}"
        )
    run_stress = run([str(linked_list_bin)])
    workloads.append(
        {
            "name": "gc_stress_linked_list",
            "build_elapsed_ns": build_stress.elapsed_ns,  # type: ignore[index]
            "run_elapsed_ns": run_stress.elapsed_ns,  # type: ignore[index]
            "exit_code": run_stress.returncode,
            "stdout": run_stress.stdout.strip(),
            "stderr": run_stress.stderr.strip(),
        }
    )

    selfhost_bin = workdir / "selfhost_bootstrap"
    build_selfhost = run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "drat",
            "--",
            "build",
            "src/main.dt",
            "-o",
            str(selfhost_bin),
        ],
        env=drat_env,
    )
    if build_selfhost.returncode != 0:
        workloads.append(
            {
                "name": "selfhost_bootstrap",
                "status": "blocked",
                "build_elapsed_ns": build_selfhost.elapsed_ns,  # type: ignore[index]
                "stderr_tail": "\n".join(build_selfhost.stderr.strip().splitlines()[-12:]),
                "reason": "current self-host bootstrap path is not stable enough to serve as a hard GC scorecard gate",
            }
        )
    else:
        workloads.append(
            {
                "name": "selfhost_bootstrap",
                "status": "ok",
                "build_elapsed_ns": build_selfhost.elapsed_ns,  # type: ignore[index]
                "artifact": str(selfhost_bin),
            }
        )
    return workloads


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Optional output path for the JSON report.",
    )
    args = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="draton-gc-scorecard-") as temp_dir:
        workdir = Path(temp_dir)
        report = {
            "repo_root": str(REPO_ROOT),
            "generated_at_epoch_ns": time.time_ns(),
            "runtime_scenarios": run_runtime_scenarios(),
            "toolchain_workloads": run_toolchain_workloads(workdir),
            "notes": [
                "Runtime scenarios are synthetic GC baselines driven directly through draton-runtime.",
                "Toolchain workloads use drat build on real repository programs to anchor the scorecard in compiler-facing behavior.",
                "The self-host bootstrap workload is reported as blocked instead of failing the whole scorecard when the current repository build path is not stable enough.",
            ],
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
