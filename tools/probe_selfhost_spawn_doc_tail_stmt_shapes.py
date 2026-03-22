#!/usr/bin/env python3
"""Probe which tail-function body shapes flip the residual spawn-doc case from SIGSEGV to OOM."""

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
        ("fn-empty", "fn tail() {}\n"),
        ("fn-semicolon", "fn tail() {\n    ;\n}\n"),
        ("fn-empty-block", "fn tail() {\n    {}\n}\n"),
        ("fn-let-literal", "fn tail() {\n    let x = 0\n}\n"),
        ("fn-let-call", "fn tail() {\n    let x = ready()\n}\n"),
        ("fn-let-grouped", "fn tail() {\n    let x = (0)\n}\n"),
        ("fn-expr-literal", "fn tail() {\n    0\n}\n"),
        ("fn-expr-call", "fn tail() {\n    ready()\n}\n"),
        ("fn-expr-grouped", "fn tail() {\n    (0)\n}\n"),
        ("fn-return-literal", "fn tail() {\n    return 0\n}\n"),
        ("fn-return-call", "fn tail() {\n    return ready()\n}\n"),
        ("fn-return-grouped", "fn tail() {\n    return (0)\n}\n"),
        ("fn-spawn-empty", "fn tail() {\n    spawn {}\n}\n"),
        ("fn-spawn-call", "fn tail() {\n    spawn ready()\n}\n"),
        ("fn-spawn-grouped", "fn tail() {\n    spawn (0)\n}\n"),
        ("fn-if-empty", "fn tail() {\n    if ready() {}\n}\n"),
        ("fn-while-empty", "fn tail() {\n    while ready() {}\n}\n"),
    ]

    print(f"stage1: {stage1}")
    results: dict[str, tuple[int, str]] = {}
    for label, tail in cases:
        code, first_line = run_text(stage1, residual + "\n" + tail)
        results[label] = (code, first_line)
        print(f"{label}: returncode={code}")
        if first_line:
            print(f"{label}: message={first_line}")

    if (
        results["fn-empty"][0] == -11
        and results["fn-semicolon"][0] == -11
        and results["fn-empty-block"][0] == -11
        and results["fn-let-literal"][0] == -11
        and results["fn-let-call"][0] == -11
        and results["fn-let-grouped"][0] == -11
        and results["fn-return-call"][0] == -11
        and results["fn-return-grouped"][0] == -11
        and results["fn-spawn-call"][0] == -11
        and results["fn-spawn-grouped"][0] == -11
        and results["fn-if-empty"][0] == -11
        and results["fn-while-empty"][0] == -11
        and results["fn-expr-literal"][0] == -11
        and results["fn-expr-call"][0] == -6
        and "memory allocation" in results["fn-expr-call"][1]
        and results["fn-expr-grouped"][0] == -6
        and "memory allocation" in results["fn-expr-grouped"][1]
        and results["fn-return-literal"][0] == -6
        and "memory allocation" in results["fn-return-literal"][1]
        and results["fn-spawn-empty"][0] == -6
        and "memory allocation" in results["fn-spawn-empty"][1]
    ):
        print(
            "summary: inside the residual both-bad + spawn-doc context, tail-function body shapes split sharply: "
            "empty/control-flow/let paths and call-or-grouped return/spawn forms stay on SIGSEGV, while bare call/grouped expression statements, "
            "literal returns, and `spawn {}` flip the later-function path into SIGABRT/OOM"
        )
        return 1

    print("summary: spawn-doc tail statement-shape pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
