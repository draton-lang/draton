#!/usr/bin/env python3
"""Capture a stable gdb backtrace for the current self-host parser crash."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent


def find_stage1() -> Path:
    candidates = [
        Path("/tmp/draton_s1"),
        REPO / "build" / "debug" / "draton-selfhost-phase1",
        REPO / "draton_selfhost",
    ]
    for path in candidates:
        if path.exists():
            return path
    raise FileNotFoundError("no stage1 self-host binary found")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    parser.add_argument(
        "--input",
        default=str(REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"),
        help="input Draton source used for the parser crash repro",
    )
    parser.add_argument(
        "--command",
        default="ast-dump",
        help="self-host subcommand to run under gdb",
    )
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    input_path = Path(args.input)

    argv = [
        "gdb",
        "-q",
        "-batch",
        "-ex",
        "set pagination off",
        "-ex",
        f"run {args.command} {input_path}",
        "-ex",
        "bt",
        "--args",
        str(stage1),
    ]
    result = subprocess.run(argv, cwd=REPO, capture_output=True, text=True)
    if result.stdout:
        print(result.stdout.strip())
    if result.stderr:
        print(result.stderr.strip())
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
