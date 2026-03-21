#!/usr/bin/env python3
"""Probe stmt3/stmt4 order and spacing sensitivity for mixed-branch crashes."""

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
NEUTRAL = "    let gap = cmd"


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
    cases = [
        ("bad3-orig4-adjacent", base.replace(STMT3, BAD3)),
        ("orig3-bad4-adjacent", base.replace(STMT4, BAD4)),
        ("bad3-neutral-orig4", base.replace(STMT3 + "\n" + STMT4, BAD3 + "\n" + NEUTRAL + "\n" + STMT4)),
        ("orig3-neutral-bad4", base.replace(STMT3 + "\n" + STMT4, STMT3 + "\n" + NEUTRAL + "\n" + BAD4)),
        ("swap-orig4-then-bad3", base.replace(STMT3 + "\n" + STMT4, STMT4 + "\n" + BAD3)),
        ("swap-bad4-then-orig3", base.replace(STMT3 + "\n" + STMT4, BAD4 + "\n" + STMT3)),
        ("swap-orig4-neutral-bad3", base.replace(STMT3 + "\n" + STMT4, STMT4 + "\n" + NEUTRAL + "\n" + BAD3)),
        ("swap-bad4-neutral-orig3", base.replace(STMT3 + "\n" + STMT4, BAD4 + "\n" + NEUTRAL + "\n" + STMT3)),
    ]

    print(f"stage1: {stage1}")
    all_fail = True
    for label, text in cases:
        code = run_text(stage1, text)
        print(f"{label}: returncode={code}")
        if code == 0:
            all_fail = False

    if all_fail:
        print(
            "summary: mixed stmt3/stmt4 branch pairs keep crashing even when their order is swapped or a neutral statement separates them"
        )
        return 1

    print("summary: stmt3/stmt4 order-spacing pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
