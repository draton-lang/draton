#!/usr/bin/env python3
"""Probe where doc comments become harmful between stmt3 and stmt4."""

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
        ("bare-doc-comment", "    /// gap"),
        ("doc-before-block", "    /// gap\n    {}"),
        ("block-then-doc", "    {}\n    /// gap"),
        ("spawn-then-doc", "    spawn {}\n    /// gap"),
        ("doc-only-plain-block", "    {\n        /// gap\n    }"),
        ("doc-only-spawn-block", "    spawn {\n        /// gap\n    }"),
        ("line-only-plain-block", "    {\n        // gap\n    }"),
        ("line-only-spawn-block", "    spawn {\n        // gap\n    }"),
    ]

    print(f"stage1: {stage1}")
    results: dict[str, int] = {}
    for label, stmt in cases:
        text = replace_stmt34(base, BAD3 + "\n" + stmt + "\n" + BAD4)
        code = run_text(stage1, text)
        results[label] = code
        print(f"{label}: returncode={code}")

    if (
        results["bare-doc-comment"] == 0
        and results["doc-before-block"] == 0
        and results["block-then-doc"] == 0
        and results["spawn-then-doc"] == 0
        and results["doc-only-plain-block"] != 0
        and results["doc-only-spawn-block"] != 0
        and results["line-only-plain-block"] == 0
        and results["line-only-spawn-block"] == 0
    ):
        print(
            "summary: doc comments only become harmful when they are the sole contents of the otherwise harmless plain/spawn empty-block separators; "
            "the same doc-comment tokens are harmless before, after, or between those separators"
        )
        return 1

    print("summary: stmt3/stmt4 doc-comment-position pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
