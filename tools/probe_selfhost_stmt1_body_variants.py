#!/usr/bin/env python3
"""Probe which statement-1 body shapes preserve the parser crash."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"
NEEDLE = "    if cli_argc() < 2 {\n        print_usage()\n        return 1\n    }"


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
    variants = [
        ("stmt1-empty-body", "    if cli_argc() < 2 {\n    }"),
        ("stmt1-print-only", "    if cli_argc() < 2 {\n        print_usage()\n    }"),
        ("stmt1-return-only", "    if cli_argc() < 2 {\n        return 1\n    }"),
        ("stmt1-let-only", "    if cli_argc() < 2 {\n        let warm = 1\n    }"),
        ("stmt1-let-call", "    if cli_argc() < 2 {\n        let warm = cli_argc()\n    }"),
        ("stmt1-expr-call", "    if cli_argc() < 2 {\n        print_usage()\n        0\n    }"),
        ("stmt1-bare-int", "    if cli_argc() < 2 {\n        0\n    }"),
        ("stmt1-bare-binary", "    if cli_argc() < 2 {\n        1 < 2\n    }"),
        ("stmt1-two-lets", "    if cli_argc() < 2 {\n        let a = 1\n        let b = 2\n    }"),
        ("stmt1-if-nested", "    if cli_argc() < 2 {\n        if ready() {\n            return 1\n        }\n    }"),
    ]

    print(f"stage1: {stage1}")
    empty_body_passes = True
    non_empty_bodies_fail = True
    for label, replacement in variants:
        code = run_text(stage1, base.replace(NEEDLE, replacement))
        print(f"{label}: returncode={code}")
        if label == "stmt1-empty-body" and code != 0:
            empty_body_passes = False
        if label != "stmt1-empty-body" and code == 0:
            non_empty_bodies_fail = False

    if empty_body_passes and non_empty_bodies_fail:
        print("summary: once stmt1 has the bad binary-condition shape, any probed non-empty body preserves the crash")
        return 1

    print("summary: stmt1 body-shape pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
