#!/usr/bin/env python3
"""Probe whether the full class and full @type payloads are both required for the parser crash."""

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

    empty_class = "class ParsedArgs {\n}\n\n"
    mini_class = (
        "class ParsedArgs {\n"
        "    let mut source_path\n\n"
        "    @type {\n"
        "        source_path: String\n"
        "    }\n"
        "}\n\n"
    )
    empty_type = "@type {\n}\n\n"
    mini_type = (
        "@type {\n"
        "    main: () -> Int\n"
        "    cmd_build: (Array[String]) -> Int\n"
        "    cmd_run: (Array[String]) -> Int\n"
        "    cli_argc: () -> Int\n"
        "    cli_arg: (Int) -> String\n"
        "    str_eq_main: (String, String) -> Bool\n"
        "    print_usage: () -> Unit\n"
        "    collect_cli_args: (Int) -> Array[String]\n"
        "}\n\n"
    )

    variants = [
        ("full", imports + klass + typeblk + main_block),
        ("empty-class", imports + empty_class + typeblk + main_block),
        ("mini-class", imports + mini_class + typeblk + main_block),
        ("empty-type", imports + klass + empty_type + main_block),
        ("mini-type", imports + klass + mini_type + main_block),
        ("mini-class+mini-type", imports + mini_class + mini_type + main_block),
    ]

    print(f"stage1: {stage1}")
    only_full_fails = True
    for label, text in variants:
        code = run_text(stage1, text)
        print(f"{label}: returncode={code}")
        if label == "full" and code == 0:
            only_full_fails = False
        if label != "full" and code != 0:
            only_full_fails = False

    if only_full_fails:
        print("summary: shrinking either the class payload or the @type payload removes the crash")
        return 1

    print("summary: header payload pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
