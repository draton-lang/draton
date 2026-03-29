#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from datetime import datetime, timezone


WORKFLOW = "ci.yml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Dispatch and optionally watch the remote Phase 1 blocker workflow via GitHub CLI."
    )
    parser.add_argument("--ref", help="Git ref to dispatch the workflow on")
    parser.add_argument(
        "--parse-slice",
        action="store_true",
        help="Run the heavier parser slice remotely",
    )
    parser.add_argument(
        "--release-smoke",
        action="store_true",
        help="Run packaged release smoke remotely",
    )
    parser.add_argument(
        "--no-wait",
        action="store_true",
        help="Dispatch without waiting for the run to finish",
    )
    parser.add_argument(
        "--poll-seconds",
        type=float,
        default=5.0,
        help="Polling interval while waiting for GitHub to create the workflow run",
    )
    parser.add_argument(
        "--discover-timeout",
        type=float,
        default=60.0,
        help="How long to wait for the dispatched run to appear",
    )
    return parser.parse_args()


def run_command(args: list[str]) -> str:
    completed = subprocess.run(
        args,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode != 0:
        raise SystemExit(
            completed.stderr.strip() or f"command failed with exit code {completed.returncode}: {' '.join(args)}"
        )
    return completed.stdout


def default_ref() -> str:
    return run_command(["git", "rev-parse", "--abbrev-ref", "HEAD"]).strip()


def workflow_inputs(args: argparse.Namespace) -> dict[str, str]:
    return {
        "parse_slice": "true" if args.parse_slice else "false",
        "release_smoke": "true" if args.release_smoke else "false",
    }


def build_workflow_run_command(ref: str, inputs: dict[str, str]) -> list[str]:
    command = ["gh", "workflow", "run", WORKFLOW, "--ref", ref]
    for key, value in inputs.items():
        command.extend(["-f", f"{key}={value}"])
    return command


def parse_iso8601(value: str) -> datetime:
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    return datetime.fromisoformat(value).astimezone(timezone.utc)


def discover_run(ref: str, dispatched_at: datetime, timeout_seconds: float, poll_seconds: float) -> dict:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        output = run_command(
            [
                "gh",
                "run",
                "list",
                "--workflow",
                WORKFLOW,
                "--branch",
                ref,
                "--event",
                "workflow_dispatch",
                "--limit",
                "10",
                "--json",
                "databaseId,createdAt,url,status,conclusion,headBranch",
            ]
        )
        runs = json.loads(output)
        for run in runs:
            if run["headBranch"] != ref:
                continue
            created_at = parse_iso8601(run["createdAt"])
            if created_at >= dispatched_at:
                return run
        time.sleep(poll_seconds)
    raise SystemExit(
        f"workflow dispatch succeeded but no {WORKFLOW} run appeared on ref {ref!r} within {timeout_seconds:.0f}s"
    )


def main() -> int:
    args = parse_args()
    ref = args.ref or default_ref()
    inputs = workflow_inputs(args)
    dispatched_at = datetime.now(timezone.utc)
    run_command(build_workflow_run_command(ref, inputs))
    run_info = discover_run(ref, dispatched_at, args.discover_timeout, args.poll_seconds)
    print(run_info["url"])
    if args.no_wait:
        return 0
    subprocess.run(["gh", "run", "watch", str(run_info["databaseId"]), "--exit-status"], check=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
