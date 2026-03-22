#!/usr/bin/env python3
"""Probe how residual spawn-doc corruption reacts to later top-level items and tail function bodies."""

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
SPAWN_DOC = '    spawn {\n        /// gap\n    }'


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


def build_residual_source() -> str:
    base = FIXTURE.read_text(encoding="utf-8")
    return base.replace(STMT3 + "\n" + STMT4, BAD3 + "\n" + SPAWN_DOC + "\n" + BAD4)


def run_text(stage1: Path, text: str) -> tuple[int, str]:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".dt", delete=False) as handle:
        handle.write(text)
        path = Path(handle.name)
    try:
        result = subprocess.run(
            [str(stage1), "ast-dump", str(path)],
            cwd=REPO,
            capture_output=True,
            text=True,
        )
        output = ((result.stdout or "") + (result.stderr or "")).strip()
        return result.returncode, output.splitlines()[0] if output else ""
    finally:
        path.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    residual = build_residual_source()
    cases = [
        ("base-residual", residual),
        ("plus-empty-class", residual + "\nclass Tail {}\n"),
        ("plus-type", residual + "\n@type { tail: () -> Int }\n"),
        ("fn-empty-body", residual + "\nfn tail() {}\n"),
        ("fn-line-body", residual + "\nfn tail() {\n    // tail\n}\n"),
        ("fn-doc-body", residual + "\nfn tail() {\n    /// tail\n}\n"),
        ("fn-return0", residual + "\nfn tail() {\n    return 0\n}\n"),
        ("fn-grouped", residual + "\nfn tail() {\n    (0)\n}\n"),
    ]

    print(f"stage1: {stage1}")
    results: dict[str, tuple[int, str]] = {}
    for label, source in cases:
        code, first_line = run_text(stage1, source)
        results[label] = (code, first_line)
        print(f"{label}: returncode={code}")
        if first_line:
            print(f"{label}: message={first_line}")

    if (
        results["base-residual"][0] == -11
        and results["plus-empty-class"][0] == -11
        and results["plus-type"][0] == -11
        and results["fn-empty-body"][0] == -11
        and results["fn-line-body"][0] == -11
        and results["fn-doc-body"][0] == -11
        and results["fn-return0"][0] == -6
        and "memory allocation" in results["fn-return0"][1]
        and results["fn-grouped"][0] == -6
        and "memory allocation" in results["fn-grouped"][1]
    ):
        print(
            "summary: the residual both-bad + spawn-doc crash stays at SIGSEGV across later class/type items and across later empty/comment-only tail functions, "
            "but flips to SIGABRT/OOM once a later top-level function has a non-empty body; this points at a second corruption path that appears only after re-entering "
            "statement parsing inside a subsequent function body"
        )
        return 1

    print("summary: spawn-doc tail-function pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
