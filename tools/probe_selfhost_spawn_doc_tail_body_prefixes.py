#!/usr/bin/env python3
"""Probe how leading harmless-looking statements change residual spawn-doc tail-function dispatch buckets."""

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
        ("none", ""),
        ("empty-block", "    {}\n"),
        ("line-comment", "    // keep\n"),
        ("blank-line", "\n"),
        ("spawn-empty", "    spawn {}\n"),
        ("let-zero", "    let warm = 0\n"),
    ]
    targets = [
        ("return-literal", "    return 0\n"),
        ("let-call", "    let x = ready()\n"),
        ("return-call", "    return ready()\n"),
    ]

    print(f"stage1: {stage1}")
    results: dict[tuple[str, str], str] = {}
    for target_label, target_stmt in targets:
        for prefix_label, prefix in prefixes:
            body = prefix + target_stmt
            source = residual + "\nfn tail() {\n" + body + "}\n"
            frames = parser_frames(stage1, source)
            bucket = classify(frames)
            results[(target_label, prefix_label)] = bucket
            print(f"{target_label}+{prefix_label}: {bucket}")
            print("  " + " -> ".join(frames[:12]))

    if (
        results[("return-literal", "none")] == "expr-dispatch"
        and results[("return-literal", "empty-block")] == "pre-dispatch"
        and results[("return-literal", "line-comment")] == "expr-dispatch"
        and results[("return-literal", "blank-line")] == "expr-dispatch"
        and results[("return-literal", "spawn-empty")] == "if-tail-dispatch"
        and results[("return-literal", "let-zero")] == "expr-dispatch"
        and results[("let-call", "none")] == "if-tail-dispatch"
        and results[("let-call", "empty-block")] == "if-tail-dispatch"
        and results[("let-call", "line-comment")] == "if-tail-dispatch"
        and results[("let-call", "blank-line")] == "if-tail-dispatch"
        and results[("let-call", "spawn-empty")] == "if-tail-dispatch"
        and results[("let-call", "let-zero")] == "if-tail-dispatch"
        and results[("return-call", "none")] == "pre-dispatch"
        and results[("return-call", "empty-block")] == "if-tail-dispatch"
        and results[("return-call", "line-comment")] == "pre-dispatch"
        and results[("return-call", "blank-line")] == "pre-dispatch"
        and results[("return-call", "spawn-empty")] == "expr-dispatch"
        and results[("return-call", "let-zero")] == "expr-dispatch"
    ):
        print(
            "summary: in the residual spawn-doc tail-function path, layout-only prefixes do not change buckets, but real leading statements do; "
            "`return 0` stays on expr-dispatch across layout-only prefixes, drops to pre-dispatch after a leading `{}`, and shifts to if-tail-dispatch only after a leading `spawn {}`; "
            "`return ready()` stays pre-dispatch across layout-only prefixes, shifts to if-tail-dispatch after a leading `{}`, and shifts to expr-dispatch after a leading `spawn {}` or `let warm = 0`; "
            "`let x = ready()` stays trapped on the if-tail path regardless of the tested prefixes"
        )
        return 1

    print("summary: spawn-doc tail body-prefix pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
