#!/usr/bin/env python3
"""Probe whether only a plain bare block is harmless between stmt3 and stmt4."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"
STMT3 = '    if str_eq_main(cmd, "build") {\n        return cmd_build(collect_cli_args(2))\n    }'
STMT4 = '    if str_eq_main(cmd, "run") {\n        return cmd_run(collect_cli_args(2))\n    }'
BAD3 = '    if foo(cmd, "build") {\n        (cmd)\n    }'
BAD4 = '    if foo(cmd, "run") {\n        (cmd)\n    }'


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


def replace_stmt34(base: str, body: str) -> str:
    return base.replace(STMT3 + "\n" + STMT4, body)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    base = FIXTURE.read_text(encoding="utf-8")
    cases = [
        ("plain-empty-block", "    {}"),
        ("empty-block-semicolon", "    {};"),
        ("unsafe-empty-block", "    @unsafe {}"),
        ("pointer-empty-block", "    @pointer {}"),
        ("comptime-empty-block", "    @comptime {}"),
        ("if-compile-empty-block", "    @if cli_argc() {}"),
        ("gc-config-empty-block", "    @gc_config {}"),
    ]

    print(f"stage1: {stage1}")
    results: dict[str, int] = {}
    for label, stmt in cases:
        text = replace_stmt34(base, BAD3 + "\n" + stmt + "\n" + BAD4)
        code = run_text(stage1, text)
        results[label] = code
        print(f"{label}: returncode={code}")

    if (
        results["plain-empty-block"] == 0
        and results["empty-block-semicolon"] != 0
        and results["unsafe-empty-block"] != 0
        and results["pointer-empty-block"] != 0
        and results["comptime-empty-block"] != 0
        and results["if-compile-empty-block"] != 0
        and results["gc-config-empty-block"] != 0
    ):
        print(
            "summary: only a plain bare `{}` block is harmless between the both-bad stmt3/stmt4 pair; "
            "adding a semicolon or any statement wrapper makes it crash again"
        )
        return 1

    print("summary: stmt3/stmt4 plain-block-only pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
