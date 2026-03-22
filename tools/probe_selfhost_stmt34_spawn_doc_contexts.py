#!/usr/bin/env python3
"""Probe where `spawn { /// ... }` still changes stmt3/stmt4 outcomes after the parse_block fix."""

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
SPAWN_EMPTY = "    spawn {}"
SPAWN_DOC = '    spawn {\n        /// gap\n    }'


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
        ("both-bad-spawn-empty", replace_stmt34(base, BAD3 + "\n" + SPAWN_EMPTY + "\n" + BAD4)),
        ("both-bad-spawn-doc", replace_stmt34(base, BAD3 + "\n" + SPAWN_DOC + "\n" + BAD4)),
        ("bad-spawn-empty-orig", replace_stmt34(base, BAD3 + "\n" + SPAWN_EMPTY + "\n" + STMT4)),
        ("bad-spawn-doc-orig", replace_stmt34(base, BAD3 + "\n" + SPAWN_DOC + "\n" + STMT4)),
        ("orig-spawn-empty-bad", replace_stmt34(base, STMT3 + "\n" + SPAWN_EMPTY + "\n" + BAD4)),
        ("orig-spawn-doc-bad", replace_stmt34(base, STMT3 + "\n" + SPAWN_DOC + "\n" + BAD4)),
        ("bad-spawn-doc-only", replace_stmt34(base, BAD3 + "\n" + SPAWN_DOC)),
        ("spawn-doc-bad-only", replace_stmt34(base, SPAWN_DOC + "\n" + BAD4)),
        ("spawn-doc-alone", replace_stmt34(base, SPAWN_DOC)),
    ]

    print(f"stage1: {stage1}")
    results: dict[str, int] = {}
    for label, text in cases:
        code = run_text(stage1, text)
        results[label] = code
        print(f"{label}: returncode={code}")

    if (
        results["both-bad-spawn-empty"] == 0
        and results["both-bad-spawn-doc"] != 0
        and results["bad-spawn-empty-orig"] != 0
        and results["bad-spawn-doc-orig"] != 0
        and results["orig-spawn-empty-bad"] != 0
        and results["orig-spawn-doc-bad"] != 0
        and results["bad-spawn-doc-only"] == 0
        and results["spawn-doc-bad-only"] == 0
        and results["spawn-doc-alone"] == 0
    ):
        print(
            "summary: after the parse_block fix, `spawn { /// gap }` only changes outcomes inside the both-bad stmt3/stmt4 pair; "
            "outside that context it behaves like an ordinary harmless separator or standalone statement"
        )
        return 1

    print("summary: stmt3/stmt4 spawn-doc context pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
