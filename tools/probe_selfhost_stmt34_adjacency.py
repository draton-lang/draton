#!/usr/bin/env python3
"""Probe whether stmt3/stmt4 crashes are fundamentally adjacency-sensitive."""

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
GAP = "    let gap = cmd"


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
        ("orig-adjacent", base),
        ("orig-gap1", replace_stmt34(base, STMT3 + "\n" + GAP + "\n" + STMT4)),
        ("mixed-adjacent", replace_stmt34(base, BAD3 + "\n" + STMT4)),
        ("mixed-gap1", replace_stmt34(base, BAD3 + "\n" + GAP + "\n" + STMT4)),
        ("both-bad-adjacent", replace_stmt34(base, BAD3 + "\n" + BAD4)),
        ("both-bad-gap1", replace_stmt34(base, BAD3 + "\n" + GAP + "\n" + BAD4)),
        ("both-bad-swap-adjacent", replace_stmt34(base, BAD4 + "\n" + BAD3)),
        ("both-bad-swap-gap1", replace_stmt34(base, BAD4 + "\n" + GAP + "\n" + BAD3)),
    ]

    print(f"stage1: {stage1}")
    results: dict[str, int] = {}
    for label, text in cases:
        code = run_text(stage1, text)
        results[label] = code
        print(f"{label}: returncode={code}")

    if (
        results["orig-adjacent"] != 0
        and results["orig-gap1"] != 0
        and results["mixed-adjacent"] != 0
        and results["mixed-gap1"] != 0
        and results["both-bad-adjacent"] == 0
        and results["both-bad-gap1"] != 0
        and results["both-bad-swap-adjacent"] == 0
        and results["both-bad-swap-gap1"] != 0
    ):
        print(
            "summary: stmt3/stmt4 adjacency is the only reason the both-bad pair passes; "
            "inserting one neutral statement makes even both-bad pairs crash"
        )
        return 1

    print("summary: stmt3/stmt4 adjacency pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
