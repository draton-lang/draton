#!/usr/bin/env python3
"""Reproduce the current self-host bootstrap blockers.

This script is intentionally narrow and operational:

- it checks whether the stage1 self-host binary crashes on `src/main.dt`
- it checks the checked-in parser repro at `tests/programs/selfhost/parser_header_plus_main.dt`
- it checks the current `examples/hello.dt` self-host build failure on Linux

It is meant as a progress harness while stage2/stage3 bootstrap is still blocked.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
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


def merged_output(result: subprocess.CompletedProcess[bytes]) -> str:
    stdout = result.stdout.decode("utf-8", "replace").strip()
    stderr = result.stderr.decode("utf-8", "replace").strip()
    if stdout and stderr:
        return f"{stderr}\n{stdout}"
    return stderr or stdout


def print_case(label: str, argv: list[str], env: dict[str, str] | None = None) -> int:
    result = subprocess.run(argv, cwd=REPO, capture_output=True, env=env)
    print(f"[{label}] returncode={result.returncode}")
    detail = merged_output(result)
    if detail:
        print(detail[:600])
    print()
    return result.returncode


def checked_in_parser_repro() -> Path:
    return REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    print(f"stage1: {stage1}")
    print()

    failures = 0

    if print_case("check-src-main", [str(stage1), "check", str(REPO / "src" / "main.dt")]) != 0:
        failures += 1

    if print_case("ast-dump-src-main", [str(stage1), "ast-dump", str(REPO / "src" / "main.dt")]) != 0:
        failures += 1

    repro = checked_in_parser_repro()
    if print_case("ast-dump-main-prefix4", [str(stage1), "ast-dump", str(repro)]) != 0:
        failures += 1

    runtime_lib = REPO / "target" / "debug" / "libdraton_runtime.a"
    env = dict(os.environ)
    env["DRATON_RUNTIME_LIB"] = str(runtime_lib)
    if print_case("build-hello", [str(stage1), "build", str(REPO / "examples" / "hello.dt"), "-o", "/tmp/selfhost_hello"], env=env) != 0:
        failures += 1

    return 0 if failures == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
