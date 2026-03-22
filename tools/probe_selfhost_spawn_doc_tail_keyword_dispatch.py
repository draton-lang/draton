#!/usr/bin/env python3
"""Probe whether keyword-led tail statements still reach their dedicated parsers in the residual spawn-doc context."""

from __future__ import annotations

import argparse
import re
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
FRAME_RE = re.compile(r"^#\d+\s+(?:0x[0-9a-f]+\s+in\s+)?([A-Za-z0-9_:#{}<>-]+)")


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


def parser_frames(stage1: Path, source: str) -> list[str]:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".dt", delete=False) as handle:
        handle.write(source)
        path = Path(handle.name)
    try:
        argv = [
            "gdb",
            "-q",
            "-batch",
            "-ex",
            "set pagination off",
            "-ex",
            f"run ast-dump {path}",
            "-ex",
            "bt",
            "--args",
            str(stage1),
        ]
        result = subprocess.run(argv, cwd=REPO, capture_output=True, text=True)
        output = ((result.stdout or "") + ("\n" + result.stderr if result.stderr else "")).strip()
        frames: list[str] = []
        for line in output.splitlines():
            match = FRAME_RE.match(line.strip())
            if match:
                frame = match.group(1)
                if frame.startswith("parser_") or frame.startswith("parse_"):
                    frames.append(frame)
        return frames
    finally:
        path.unlink(missing_ok=True)


def classify(frames: list[str]) -> str:
    if "parse_return_stmt" in frames:
        return "return-dispatch"
    if "parse_spawn_stmt" in frames:
        return "spawn-dispatch"
    if "parse_let_stmt" in frames:
        return "let-dispatch"
    if "parse_if_stmt_tail" in frames:
        return "if-tail-dispatch"
    if "parse_expr_stmt_or_assignment" in frames:
        return "expr-dispatch"
    if "parse_block" in frames and "parse_stmt" not in frames:
        return "pre-dispatch"
    return "other"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    residual = build_residual_source()
    cases = [
        ("return-literal", "fn tail() {\n    return 0\n}\n"),
        ("return-call", "fn tail() {\n    return ready()\n}\n"),
        ("return-grouped", "fn tail() {\n    return (0)\n}\n"),
        ("spawn-empty", "fn tail() {\n    spawn {}\n}\n"),
        ("spawn-call", "fn tail() {\n    spawn ready()\n}\n"),
        ("spawn-grouped", "fn tail() {\n    spawn (0)\n}\n"),
        ("let-literal", "fn tail() {\n    let x = 0\n}\n"),
        ("let-call", "fn tail() {\n    let x = ready()\n}\n"),
        ("let-grouped", "fn tail() {\n    let x = (0)\n}\n"),
    ]

    results: dict[str, tuple[str, list[str]]] = {}
    print(f"stage1: {stage1}")
    for label, tail in cases:
        frames = parser_frames(stage1, residual + "\n" + tail)
        category = classify(frames)
        results[label] = (category, frames)
        print(f"{label}: {category}")
        print("  " + " -> ".join(frames[:12]))

    if (
        results["return-literal"][0] == "expr-dispatch"
        and results["spawn-empty"][0] == "expr-dispatch"
        and results["return-call"][0] == "pre-dispatch"
        and results["return-grouped"][0] == "pre-dispatch"
        and results["spawn-call"][0] == "pre-dispatch"
        and results["spawn-grouped"][0] == "pre-dispatch"
        and results["let-literal"][0] == "pre-dispatch"
        and results["let-call"][0] == "if-tail-dispatch"
        and results["let-grouped"][0] == "if-tail-dispatch"
        and all(category not in {"return-dispatch", "spawn-dispatch", "let-dispatch"} for category, _frames in results.values())
    ):
        print(
            "summary: in the residual spawn-doc tail-function context, none of the probed keyword-led statements still reach their dedicated parsers; "
            "`return 0` and `spawn {}` now fall through the expr-statement sink, `let x = ready()` / `let x = (0)` are misrouted into `parse_if_stmt_tail`, and the remaining probed return/spawn/let variants die earlier before `parse_stmt` dispatch"
        )
        return 1

    print("summary: spawn-doc tail keyword-dispatch pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
