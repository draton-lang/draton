#!/usr/bin/env python3
"""Probe subset interactions inside the first four statements of `src/main.dt::main()`."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path

from probe_selfhost_main_prefixes import extract_header_and_main_lines, split_main_statements


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


def build_subset_fixture(header: str, statements: list[list[str]], indexes: list[int]) -> str:
    body = ["fn main() {"]
    for index in indexes:
        body.extend(statements[index - 1])
    body.append("}")
    return header + "\n".join(body) + "\n"


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

    cases = [
        [3],
        [4],
        [2, 3],
        [2, 4],
        [3, 4],
        [1, 3],
        [1, 4],
        [1, 2, 3],
        [1, 2, 4],
        [2, 3, 4],
        [1, 3, 4],
        [1, 2, 3, 4],
    ]
    labels = {
        1: "stmt1_if_argc",
        2: "stmt2_let_cmd",
        3: "stmt3_if_build",
        4: "stmt4_if_run",
    }

    print(f"stage1: {stage1}")
    first_failure = ""
    for case in cases:
        label = "+".join(labels[index] for index in case)
        code = run_case(stage1, build_subset_fixture(header, statements, case))
        print(f"{label}: returncode={code}")
        if code != 0 and first_failure == "":
            first_failure = label

    if first_failure != "":
        print(f"first failing subset: {first_failure}")
        return 1

    print("all probed subsets passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
