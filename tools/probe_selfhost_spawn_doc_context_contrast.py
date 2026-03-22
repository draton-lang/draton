#!/usr/bin/env python3
"""Contrast representative tail-body prefix pairs across original, both-bad, and residual contexts."""

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


def contexts() -> dict[str, str]:
    base = FIXTURE.read_text(encoding="utf-8")
    return {
        "orig": base,
        "both-bad": base.replace(STMT3 + "\n" + STMT4, BAD3 + "\n" + BAD4),
        "residual": base.replace(STMT3 + "\n" + STMT4, BAD3 + "\n" + SPAWN_DOC + "\n" + BAD4),
    }


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
    first_parse = ""
    for frame in frames:
        if frame.startswith("parse_"):
            first_parse = frame
            break
    if first_parse == "parse_if_stmt_tail":
        return "if-tail-dispatch"
    if first_parse == "parse_expr_stmt_or_assignment":
        return "expr-dispatch"
    if first_parse == "parse_block":
        return "pre-dispatch"
    return "expression-stack"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    tails = {
        "return-call-after-empty-block": "fn tail() {\n    {}\n    return ready()\n}\n",
        "return-call-after-spawn-empty": "fn tail() {\n    spawn {}\n    return ready()\n}\n",
        "let-call-after-let-zero": "fn tail() {\n    let warm = 0\n    let x = ready()\n}\n",
    }

    print(f"stage1: {stage1}")
    results: dict[tuple[str, str], str] = {}
    for ctx_label, ctx_src in contexts().items():
        for tail_label, tail_src in tails.items():
            frames = parser_frames(stage1, ctx_src + "\n" + tail_src)
            bucket = classify(frames)
            results[(ctx_label, tail_label)] = bucket
            print(f"{ctx_label}+{tail_label}: {bucket}")
            print("  " + " -> ".join(frames[:12]))

    if (
        results[("orig", "return-call-after-empty-block")] == "expression-stack"
        and results[("orig", "return-call-after-spawn-empty")] == "expression-stack"
        and results[("orig", "let-call-after-let-zero")] == "expression-stack"
        and results[("both-bad", "return-call-after-empty-block")] == "pre-dispatch"
        and results[("both-bad", "return-call-after-spawn-empty")] == "pre-dispatch"
        and results[("both-bad", "let-call-after-let-zero")] == "expr-dispatch"
        and results[("residual", "return-call-after-empty-block")] == "if-tail-dispatch"
        and results[("residual", "return-call-after-spawn-empty")] == "expression-stack"
        and results[("residual", "let-call-after-let-zero")] == "pre-dispatch"
    ):
        print(
            "summary: the same representative tail-body prefix pairs fall into different parser sinks across original, both-bad, and residual contexts; "
            "the two-statement tail-body state machine is therefore not a generic parser quirk, but a context-dependent effect that only fully emerges after the spawn-doc poisoning step"
        )
        return 1

    print("summary: spawn-doc context-contrast pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
