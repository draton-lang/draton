#!/usr/bin/env python3
"""Probe return-expression variants inside the crashing `parser_main_prefix4` fixture."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"
NEEDLE = "        return cmd_build(collect_cli_args(2))"


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


def run_text(stage1: Path, text: str) -> int:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".dt", delete=False) as handle:
        handle.write(text)
        path = Path(handle.name)
    try:
        result = subprocess.run(
            [str(stage1), "ast-dump", str(path)],
            cwd=REPO,
            capture_output=True,
        )
        return result.returncode
    finally:
        path.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    base = FIXTURE.read_text(encoding="utf-8")
    variants = [
        ("ret-0", "        return 0"),
        ("ret-cmd-build-zero", "        return cmd_build(0)"),
        ("ret-cmd-build-cli-argc", "        return cmd_build(cli_argc())"),
        ("ret-collect", "        return collect_cli_args(2)"),
        ("ret-cmd-build-collect", "        return cmd_build(collect_cli_args(2))"),
        ("ret-cmd-build-cli-arg", "        return cmd_build(cli_arg(1))"),
        ("ret-nested-ident", "        return foo(bar(2))"),
        ("ret-single-call-ident", "        return foo(2)"),
    ]

    print(f"stage1: {stage1}")
    only_non_call_passes = True
    for label, replacement in variants:
        code = run_text(stage1, base.replace(NEEDLE, replacement))
        print(f"{label}: returncode={code}")
        if label == "ret-0" and code != 0:
            only_non_call_passes = False
        if label != "ret-0" and code == 0:
            only_non_call_passes = False

    if only_non_call_passes:
        print("summary: only the non-call return variant passes inside parser_main_prefix4")
        return 1

    print("summary: return-call probe no longer matches the current interaction pattern")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
