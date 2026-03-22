#!/usr/bin/env python3
"""Probe minimal standalone doc-comment-only block parsing failures."""

from __future__ import annotations

import argparse
import subprocess
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


def run_source(stage1: Path, source: str) -> tuple[int, str]:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".dt", delete=False) as handle:
        handle.write(source)
        path = Path(handle.name)
    try:
        result = subprocess.run(
            [str(stage1), "ast-dump", str(path)],
            cwd=REPO,
            capture_output=True,
            text=True,
        )
        output = (result.stdout or "") + (result.stderr or "")
        return result.returncode, output.strip()
    finally:
        path.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    cases = [
        ("plain-empty-block", "fn main() {\n    {}\n}\n"),
        ("plain-line-comment-block", "fn main() {\n    {\n        // gap\n    }\n}\n"),
        ("plain-doc-comment-block", "fn main() {\n    {\n        /// gap\n    }\n}\n"),
        ("spawn-empty-block", "fn main() {\n    spawn {}\n}\n"),
        ("spawn-line-comment-block", "fn main() {\n    spawn {\n        // gap\n    }\n}\n"),
        ("spawn-doc-comment-block", "fn main() {\n    spawn {\n        /// gap\n    }\n}\n"),
    ]

    print(f"stage1: {stage1}")
    results: dict[str, tuple[int, str]] = {}
    for label, source in cases:
        code, output = run_source(stage1, source)
        results[label] = (code, output)
        print(f"{label}: returncode={code}")
        if output:
            first_line = output.splitlines()[0]
            print(f"{label}: message={first_line}")

    plain_doc = results["plain-doc-comment-block"]
    spawn_doc = results["spawn-doc-comment-block"]
    if (
        results["plain-empty-block"][0] == 0
        and results["plain-line-comment-block"][0] == 0
        and plain_doc[0] == 1
        and "invalid expression" in plain_doc[1]
        and results["spawn-empty-block"][0] == 0
        and results["spawn-line-comment-block"][0] == 0
        and spawn_doc[0] == 1
        and "invalid expression" in spawn_doc[1]
    ):
        print(
            "summary: minimal standalone doc-comment-only plain/spawn blocks fail with `invalid expression`, "
            "while empty and line-comment-only variants still parse"
        )
        return 1

    print("summary: standalone doc-comment-only block pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
