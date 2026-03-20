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
        ("stmt1-let-binary", "    let warmup = 1 < 2"),
        ("stmt1-let-binary-call", "    let warmup = cli_argc() < 2"),
        ("stmt1-bare-binary", "    1 < 2"),
        ("stmt1-bare-binary-call", "    cli_argc() < 2"),
        ("stmt1-bare-add", "    cli_argc() + 2"),
        ("stmt1-if-paren-ident", "    if (cli_argc()) {\n        return 1\n    }"),
        ("stmt1-if-paren-binary", "    if (cli_argc() < 2) {\n        return 1\n    }"),
        ("stmt1-if-binary-empty", "    if cli_argc() < 2 {\n    }"),
        ("stmt1-if-1-lt-2", "    if 1 < 2 {\n        print_usage()\n        return 1\n    }"),
        ("stmt1-if-cli_argc-lt-cli_argc", "    if cli_argc() < cli_argc() {\n        print_usage()\n        return 1\n    }"),
        ("stmt1-if-cli_argc-gt-2", "    if cli_argc() > 2 {\n        print_usage()\n        return 1\n    }"),
        ("stmt1-if-cli_argc-eqeq-2", "    if cli_argc() == 2 {\n        print_usage()\n        return 1\n    }"),
        ("stmt1-if-cli_argc-plus-2", "    if cli_argc() + 2 {\n        print_usage()\n        return 1\n    }"),
    ]

    print(f"stage1: {stage1}")
    expected_pass_labels = {
        "stmt1-delete",
        "stmt1-if-ready-return1",
        "stmt1-if-cli_argc-return1",
        "stmt1-let-dummy",
        "stmt1-let-binary",
        "stmt1-let-binary-call",
        "stmt1-bare-binary",
        "stmt1-bare-binary-call",
        "stmt1-bare-add",
        "stmt1-if-paren-ident",
        "stmt1-if-binary-empty",
    }
    expected_fail_labels = {
        "stmt1-if-cli_argc-lt2-return0",
        "stmt1-if-cli_argc-lt2-no-return",
        "stmt1-if-paren-binary",
        "stmt1-if-1-lt-2",
        "stmt1-if-cli_argc-lt-cli_argc",
        "stmt1-if-cli_argc-gt-2",
        "stmt1-if-cli_argc-eqeq-2",
        "stmt1-if-cli_argc-plus-2",
    }
    pass_labels_pass = True
    fail_labels_fail = True

    for label, replacement in variants:
        code = run_text(stage1, base.replace(NEEDLE, replacement))
        print(f"{label}: returncode={code}")
        if label in expected_pass_labels and code != 0:
            pass_labels_pass = False
        if label in expected_fail_labels and code == 0:
            fail_labels_fail = False

    if pass_labels_pass and fail_labels_fail:
        print("summary: stmt1 only preserves the crash when a binary-expression condition appears inside an if with a non-empty body")
        return 1

    print("summary: stmt1 variant pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
