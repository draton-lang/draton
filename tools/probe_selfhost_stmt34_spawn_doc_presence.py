#!/usr/bin/env python3
"""Probe whether any doc-comment presence inside spawn blocks is enough to restore the both-bad crash."""

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


def replace_stmt34(base: str, middle: str) -> str:
    body = BAD3 + "\n" + middle + "\n" + BAD4
    return base.replace(STMT3 + "\n" + STMT4, body)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    base = FIXTURE.read_text(encoding="utf-8")
    cases = [
        ("spawn-empty", "    spawn {}"),
        ("spawn-line-comment", "    spawn {\n        // gap\n    }"),
        ("spawn-doc-only", "    spawn {\n        /// gap\n    }"),
        ("spawn-doc-then-line", "    spawn {\n        /// gap\n        // keep\n    }"),
        ("spawn-line-then-doc", "    spawn {\n        // keep\n        /// gap\n    }"),
        ("spawn-doc-then-empty-block", "    spawn {\n        /// gap\n        {}\n    }"),
        ("spawn-doc-then-let", "    spawn {\n        /// gap\n        let gap = cmd\n    }"),
        ("spawn-doc-then-expr", "    spawn {\n        /// gap\n        cmd\n    }"),
    ]

    print(f"stage1: {stage1}")
    results: dict[str, int] = {}
    for label, middle in cases:
        code = run_text(stage1, replace_stmt34(base, middle))
        results[label] = code
        print(f"{label}: returncode={code}")

    if (
        results["spawn-empty"] == 0
        and results["spawn-line-comment"] == 0
        and results["spawn-doc-only"] != 0
        and results["spawn-doc-then-line"] != 0
        and results["spawn-line-then-doc"] != 0
        and results["spawn-doc-then-empty-block"] != 0
        and results["spawn-doc-then-let"] != 0
        and results["spawn-doc-then-expr"] != 0
    ):
        print(
            "summary: inside the both-bad stmt3/stmt4 context, any probed doc-comment presence inside `spawn { ... }` is enough to restore the crash; "
            "line-comment-only and doc-comment-free spawn blocks still pass"
        )
        return 1

    print("summary: stmt3/stmt4 spawn-doc presence pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
