#!/usr/bin/env python3
"""Probe which first-statement shapes are necessary for the `parser_main_prefix4` crash."""

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
        ("stmt1-delete", ""),
        ("stmt1-if-ready-return1", "    if ready() {\n        return 1\n    }"),
        ("stmt1-if-cli_argc-return1", "    if cli_argc() {\n        return 1\n    }"),
        ("stmt1-if-cli_argc-lt2-return0", "    if cli_argc() < 2 {\n        return 0\n    }"),
        ("stmt1-if-cli_argc-lt2-no-return", "    if cli_argc() < 2 {\n        print_usage()\n    }"),
        ("stmt1-let-dummy", "    let argc = cli_argc()"),
        ("stmt1-if-1-lt-2", "    if 1 < 2 {\n        print_usage()\n        return 1\n    }"),
        ("stmt1-if-cli_argc-lt-cli_argc", "    if cli_argc() < cli_argc() {\n        print_usage()\n        return 1\n    }"),
        ("stmt1-if-cli_argc-gt-2", "    if cli_argc() > 2 {\n        print_usage()\n        return 1\n    }"),
        ("stmt1-if-cli_argc-eqeq-2", "    if cli_argc() == 2 {\n        print_usage()\n        return 1\n    }"),
        ("stmt1-if-cli_argc-plus-2", "    if cli_argc() + 2 {\n        print_usage()\n        return 1\n    }"),
    ]

    print(f"stage1: {stage1}")
    simple_variants_pass = True
    binary_variants_fail = True
    simple_labels = {
        "stmt1-delete",
        "stmt1-if-ready-return1",
        "stmt1-if-cli_argc-return1",
        "stmt1-let-dummy",
    }

    for label, replacement in variants:
        code = run_text(stage1, base.replace(NEEDLE, replacement))
        print(f"{label}: returncode={code}")
        if label in simple_labels and code != 0:
            simple_variants_pass = False
        if label not in simple_labels and code == 0:
            binary_variants_fail = False

    if simple_variants_pass and binary_variants_fail:
        print("summary: simple stmt1 variants pass, but stmt1 binary-expression variants preserve the crash")
        return 1

    print("summary: stmt1 variant pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
