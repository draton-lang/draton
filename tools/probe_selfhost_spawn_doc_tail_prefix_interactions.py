#!/usr/bin/env python3
"""Probe whether residual spawn-doc tail-body prefixes are harmful on their own or only by interaction with a following statement."""

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
    if "parse_if_stmt_tail" in frames:
        return "if-tail-dispatch"
    if "parse_expr_stmt_or_assignment" in frames:
        return "expr-dispatch"
    if "parse_block" in frames and "parse_stmt" not in frames:
        return "pre-dispatch"
    if "parse_stmt" in frames:
        return "stmt-dispatch"
    return "other"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    residual = build_residual_source()
    prefixes = [
        ("empty-block", "    {}\n"),
        ("spawn-empty", "    spawn {}\n"),
        ("let-zero", "    let warm = 0\n"),
    ]
    targets = [
        ("return-literal", "    return 0\n"),
        ("return-call", "    return ready()\n"),
        ("let-call", "    let x = ready()\n"),
    ]

    print(f"stage1: {stage1}")
    results: dict[tuple[str, str], str] = {}
    for prefix_label, prefix_stmt in prefixes:
        frames = parser_frames(stage1, residual + "\nfn tail() {\n" + prefix_stmt + "}\n")
        bucket = classify(frames)
        results[(prefix_label, "alone")] = bucket
        print(f"{prefix_label}+alone: {bucket}")
        print("  " + " -> ".join(frames[:12]))
        for target_label, target_stmt in targets:
            body = prefix_stmt + target_stmt
            frames = parser_frames(stage1, residual + "\nfn tail() {\n" + body + "}\n")
            bucket = classify(frames)
            results[(prefix_label, target_label)] = bucket
            print(f"{prefix_label}+{target_label}: {bucket}")
            print("  " + " -> ".join(frames[:12]))

    if (
        results[("empty-block", "alone")] == "pre-dispatch"
        and results[("empty-block", "return-literal")] == "pre-dispatch"
        and results[("empty-block", "return-call")] == "if-tail-dispatch"
        and results[("empty-block", "let-call")] == "if-tail-dispatch"
        and results[("spawn-empty", "alone")] == "expr-dispatch"
        and results[("spawn-empty", "return-literal")] == "if-tail-dispatch"
        and results[("spawn-empty", "return-call")] == "expr-dispatch"
        and results[("spawn-empty", "let-call")] == "if-tail-dispatch"
        and results[("let-zero", "alone")] == "pre-dispatch"
        and results[("let-zero", "return-literal")] == "expr-dispatch"
        and results[("let-zero", "return-call")] == "expr-dispatch"
        and results[("let-zero", "let-call")] == "if-tail-dispatch"
    ):
        print(
            "summary: in the residual spawn-doc tail-function path, the tested prefixes are not uniformly harmful on their own; "
            "`{}` and `let warm = 0` alone stay pre-dispatch, while `spawn {}` alone already hits expr-dispatch, and each prefix interacts differently with the next statement. "
            "This is now a genuine two-statement state machine, not just a property of the first statement or the second statement in isolation"
        )
        return 1

    print("summary: spawn-doc tail prefix-interaction pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
