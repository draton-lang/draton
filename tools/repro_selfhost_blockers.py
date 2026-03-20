#!/usr/bin/env python3
"""Reproduce the current self-host bootstrap blockers.

This script is intentionally narrow and operational:

- it checks whether the stage1 self-host binary crashes on `src/main.dt`
- it checks a smaller extracted repro from `src/main.dt`
- it checks the current `examples/hello.dt` self-host build failure on Linux

It is meant as a progress harness while stage2/stage3 bootstrap is still blocked.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
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


def write_header_plus_main_repro() -> Path:
    lines = (REPO / "src" / "main.dt").read_text(encoding="utf-8").splitlines()
    header = "\n".join(lines[:47]) + "\n"
    start = next(i for i, line in enumerate(lines) if line.startswith("fn main("))
    end = next(i for i, line in enumerate(lines[start + 1 :], start + 1) if line.startswith("fn cmd_build("))
    body = "\n".join(lines[start:end]) + "\n"

    handle = tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        suffix=".dt",
        delete=False,
    )
    with handle:
        handle.write(header)
        handle.write(body)
    return Path(handle.name)


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

    repro = write_header_plus_main_repro()
    try:
        if print_case("ast-dump-header-plus-main", [str(stage1), "ast-dump", str(repro)]) != 0:
            failures += 1
    finally:
        repro.unlink(missing_ok=True)

    runtime_lib = REPO / "target" / "debug" / "libdraton_runtime.a"
    env = dict(os.environ)
    env["DRATON_RUNTIME_LIB"] = str(runtime_lib)
    if print_case("build-hello", [str(stage1), "build", str(REPO / "examples" / "hello.dt"), "-o", "/tmp/selfhost_hello"], env=env) != 0:
        failures += 1

    return 0 if failures == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
