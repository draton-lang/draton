#!/usr/bin/env python3
"""Probe which header sections are necessary for the `parser_main_prefix4` crash."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"


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
    lines = FIXTURE.read_text(encoding="utf-8").splitlines()
    imports = "\n".join(lines[0:5]) + "\n\n"
    klass = "\n".join(lines[6:21]) + "\n\n"
    typeblk = "\n".join(lines[22:46]) + "\n\n"
    main_block = "\n".join(lines[47:]) + "\n"

    sections = [
        ("main-only", main_block),
        ("imports+main", imports + main_block),
        ("class+main", klass + main_block),
        ("type+main", typeblk + main_block),
        ("imports+class+main", imports + klass + main_block),
        ("imports+type+main", imports + typeblk + main_block),
        ("class+type+main", klass + typeblk + main_block),
        ("full", imports + klass + typeblk + main_block),
    ]

    print(f"stage1: {stage1}")
    only_full_fails = True
    for label, text in sections:
        code = run_text(stage1, text)
        print(f"{label}: returncode={code}")
        if label == "full" and code == 0:
            only_full_fails = False
        if label != "full" and code != 0:
            only_full_fails = False

    if only_full_fails:
        print("summary: only the full imports+class+type+main fixture fails")
        return 1

    print("summary: header dependency pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
