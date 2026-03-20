#!/usr/bin/env python3
"""Probe which prefix of `src/main.dt::main()` first crashes the self-host parser."""

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


def extract_header_and_main_lines() -> tuple[str, list[str]]:
    lines = (REPO / "src" / "main.dt").read_text(encoding="utf-8").splitlines()
    header = "\n".join(lines[:47]) + "\n"
    start = next(i for i, line in enumerate(lines) if line.startswith("fn main("))
    end = next(i for i, line in enumerate(lines[start + 1 :], start + 1) if line.startswith("fn cmd_build("))
    return header, lines[start:end]


def split_main_statements(main_lines: list[str]) -> list[list[str]]:
    statements: list[list[str]] = []
    current: list[str] = []
    body_depth = 0
    in_body = False

    for line in main_lines:
        if not in_body:
            current.append(line)
            if "{" in line:
                body_depth += line.count("{") - line.count("}")
                if body_depth > 0:
                    in_body = True
                    current = []
            continue

        if line == "}":
            if current:
                statements.append(current)
            break

        if not current:
            current = [line]
        else:
            current.append(line)

        body_depth += line.count("{") - line.count("}")
        if body_depth == 1:
            statements.append(current)
            current = []

    return statements


def build_prefix_fixture(header: str, statements: list[list[str]], count: int) -> str:
    body_lines = ["fn main() {"]
    for statement in statements[:count]:
        body_lines.extend(statement)
    body_lines.append("}")
    return header + "\n".join(body_lines) + "\n"


def run_case(stage1: Path, fixture_text: str) -> int:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".dt", delete=False) as handle:
        handle.write(fixture_text)
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
    header, main_lines = extract_header_and_main_lines()
    statements = split_main_statements(main_lines)

    print(f"stage1: {stage1}")
    print(f"top-level main statements: {len(statements)}")

    first_failure = -1
    for i in range(1, len(statements) + 1):
        fixture = build_prefix_fixture(header, statements, i)
        code = run_case(stage1, fixture)
        print(f"prefix-{i}: returncode={code}")
        if code != 0 and first_failure < 0:
            first_failure = i

    if first_failure >= 0:
        print(f"first failing prefix: {first_failure}")
        return 1

    print("all prefixes passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
