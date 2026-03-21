#!/usr/bin/env python3
"""Probe whether any ordinary intervening statement is enough to flip stmt3/stmt4 outcomes."""

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
    interveners = [
        ("let-ident", "    let gap = cmd"),
        ("let-grouped", "    let gap = (cmd)"),
        ("expr-ident", "    cmd"),
        ("expr-grouped", "    (cmd)"),
        ("call-zero", "    cli_argc()"),
        ("call-one", "    cli_arg(1)"),
        ("if-empty-call-cond", "    if cli_argc() {}"),
        ("if-empty-binary-cond", "    if 1 < 2 {}"),
    ]

    print(f"stage1: {stage1}")
    all_fail = True
    for label, stmt in interveners:
        text = replace_stmt34(base, BAD3 + "\n" + stmt + "\n" + BAD4)
        code = run_text(stage1, text)
        print(f"{label}: returncode={code}")
        if code == 0:
            all_fail = False

    if all_fail:
        print(
            "summary: any probed intervening statement is enough to make the both-bad stmt3/stmt4 pair crash; "
            "this now looks keyed to statement boundaries rather than a special neutral statement shape"
        )
        return 1

    print("summary: stmt3/stmt4 intervening-statement pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
