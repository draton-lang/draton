#!/usr/bin/env python3
"""Probe whether stmt3/stmt4 failures require one original branch to remain in place."""

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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    base = FIXTURE.read_text(encoding="utf-8")
    cases = [
        ("orig", base),
        ("replace-stmt3-only-keep-orig-stmt4", base.replace(STMT3, BAD3)),
        ("replace-stmt4-only-keep-orig-stmt3", base.replace(STMT4, BAD4)),
        ("replace-both", base.replace(STMT3, BAD3).replace(STMT4, BAD4)),
        ("bad3-only-stmt4-delete", base.replace(STMT3, BAD3).replace(STMT4, "")),
        ("bad4-only-stmt3-delete", base.replace(STMT3, "").replace(STMT4, BAD4)),
    ]

    print(f"stage1: {stage1}")
    orig_fails = False
    mixed_fail = True
    both_bad_pass = False
    bad_alone_pass = True
    for label, text in cases:
        code = run_text(stage1, text)
        print(f"{label}: returncode={code}")
        if label == "orig" and code != 0:
            orig_fails = True
        if label in {"replace-stmt3-only-keep-orig-stmt4", "replace-stmt4-only-keep-orig-stmt3"} and code == 0:
            mixed_fail = False
        if label == "replace-both" and code == 0:
            both_bad_pass = True
        if label in {"bad3-only-stmt4-delete", "bad4-only-stmt3-delete"} and code != 0:
            bad_alone_pass = False

    if orig_fails and mixed_fail and both_bad_pass and bad_alone_pass:
        print(
            "summary: one bad stmt3/stmt4 branch is enough only while the sibling branch remains in its original crashing form; "
            "replacing both branches or deleting the sibling clears the crash"
        )
        return 1

    print("summary: stmt3/stmt4 branch-dependency pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
