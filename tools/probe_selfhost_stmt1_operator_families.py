#!/usr/bin/env python3
"""Probe whether the statement-1 crash depends on a specific binary-operator family."""

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
        ("lt", "    if cli_argc() < 2 {\n        print_usage()\n        return 1\n    }"),
        ("gt", "    if cli_argc() > 2 {\n        print_usage()\n        return 1\n    }"),
        ("eqeq", "    if cli_argc() == 2 {\n        print_usage()\n        return 1\n    }"),
        ("plus", "    if cli_argc() + 2 {\n        print_usage()\n        return 1\n    }"),
        ("minus", "    if cli_argc() - 2 {\n        print_usage()\n        return 1\n    }"),
        ("mul", "    if cli_argc() * 2 {\n        print_usage()\n        return 1\n    }"),
        ("div", "    if cli_argc() / 2 {\n        print_usage()\n        return 1\n    }"),
        ("mod", "    if cli_argc() % 2 {\n        print_usage()\n        return 1\n    }"),
        ("bitand", "    if cli_argc() & 2 {\n        print_usage()\n        return 1\n    }"),
        ("bitor", "    if cli_argc() | 2 {\n        print_usage()\n        return 1\n    }"),
        ("xor", "    if cli_argc() ^ 2 {\n        print_usage()\n        return 1\n    }"),
        ("shl", "    if cli_argc() << 1 {\n        print_usage()\n        return 1\n    }"),
        ("shr", "    if cli_argc() >> 1 {\n        print_usage()\n        return 1\n    }"),
        ("andand", "    if cli_argc() && 2 {\n        print_usage()\n        return 1\n    }"),
        ("oror", "    if cli_argc() || 2 {\n        print_usage()\n        return 1\n    }"),
        ("range", "    if cli_argc() .. 2 {\n        print_usage()\n        return 1\n    }"),
    ]

    print(f"stage1: {stage1}")
    all_fail = True
    for label, replacement in variants:
        code = run_text(stage1, base.replace(NEEDLE, replacement))
        print(f"{label}: returncode={code}")
        if code == 0:
            all_fail = False

    if all_fail:
        print("summary: all probed binary-operator families preserve the crash in stmt1")
        return 1

    print("summary: at least one binary-operator family no longer preserves the crash")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
