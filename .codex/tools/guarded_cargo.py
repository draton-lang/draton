#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import shlex
import shutil
import subprocess
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
RUN_GUARDED = SCRIPT_DIR / "run_guarded.py"

PRESETS = {
    "parser": {"timeout": 900, "memory": 2048, "cpu": 600, "args": ["test", "-p", "draton-parser", "--test", "items"]},
    "typeck": {"timeout": 900, "memory": 2048, "cpu": 600, "args": ["test", "-p", "draton-typeck", "--test", "errors"]},
    "selfhost-stage0": {
        "timeout": 1800,
        "memory": 4096,
        "cpu": 1200,
        "file_size": 2048,
        "args": ["test", "-p", "drat", "--test", "selfhost_stage0", "--", "--nocapture"],
    },
    "workspace-test": {"timeout": 1800, "memory": 4096, "cpu": 1200, "args": ["test", "--workspace"]},
    "workspace-build": {"timeout": 1800, "memory": 4096, "cpu": 1200, "args": ["build", "--workspace"]},
    "workspace-clippy": {"timeout": 1800, "memory": 4096, "cpu": 1200, "args": ["clippy", "--workspace", "--", "-D", "warnings"]},
    "workspace-fmt-check": {"timeout": 600, "memory": 1024, "cpu": 300, "args": ["fmt", "--all", "--check"]},
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run common cargo workflows through run_guarded.py presets.")
    parser.add_argument("--cwd", default=".")
    parser.add_argument("--preset", choices=sorted(PRESETS))
    parser.add_argument("--timeout-sec", type=int)
    parser.add_argument("--memory-mb", type=int)
    parser.add_argument("--cpu-seconds", type=int)
    parser.add_argument("--file-size-mb", type=int)
    parser.add_argument("--concurrency", type=int, default=2)
    parser.add_argument("--wait-sec", type=int, default=120)
    parser.add_argument("--json-only", action="store_true")
    parser.add_argument("cargo_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    while args.cargo_args and args.cargo_args[0] == "--":
        args.cargo_args = args.cargo_args[1:]
    if args.preset is None and not args.cargo_args:
        parser.error("provide --preset or cargo args")
    return args


def resolve_cargo() -> str:
    explicit = os.environ.get("CARGO")
    if explicit:
        candidate = Path(explicit).expanduser()
        if candidate.exists():
            return str(candidate)
    for candidate in (
        Path.home() / ".cargo" / "bin" / "cargo",
        Path("/home/lehungquangminh/.cargo/bin/cargo"),
    ):
        if candidate.exists():
            return str(candidate)
    found = shutil.which("cargo")
    if found:
        return found
    raise SystemExit("cargo executable not found; set CARGO or install cargo in PATH")


def main() -> int:
    args = parse_args()
    if args.preset is not None:
        preset = PRESETS[args.preset]
        cargo_args = preset["args"]
        timeout = args.timeout_sec or preset["timeout"]
        memory = args.memory_mb or preset["memory"]
        cpu = args.cpu_seconds or preset["cpu"]
        file_size = args.file_size_mb or preset.get("file_size", 64)
    else:
        cargo_args = args.cargo_args
        timeout = args.timeout_sec or 1200
        memory = args.memory_mb or 3072
        cpu = args.cpu_seconds or 900
        file_size = args.file_size_mb or 64
    command = [
        sys.executable,
        str(RUN_GUARDED),
        "--cwd",
        args.cwd,
        "--timeout-sec",
        str(timeout),
        "--memory-mb",
        str(memory),
        "--cpu-seconds",
        str(cpu),
        "--file-size-mb",
        str(file_size),
        "--concurrency",
        str(args.concurrency),
        "--wait-sec",
        str(args.wait_sec),
    ]
    if args.json_only:
        command.append("--json-only")
    command.extend(["--", resolve_cargo(), *cargo_args])
    return subprocess.call(command)


if __name__ == "__main__":
    raise SystemExit(main())
