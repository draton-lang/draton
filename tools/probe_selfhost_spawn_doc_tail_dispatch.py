#!/usr/bin/env python3
"""Probe whether residual spawn-doc tail variants reach statement-dispatch paths or die before them."""

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


def gdb_parser_frames(stage1: Path, source: str) -> list[str]:
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
        frames: list[str] = []
        output = ((result.stdout or "") + ("\n" + result.stderr if result.stderr else "")).strip()
        for line in output.splitlines():
            match = FRAME_RE.match(line.strip())
            if match:
                frame = match.group(1)
                if frame.startswith("parser_") or frame.startswith("parse_"):
                    frames.append(frame)
        return frames
    finally:
        path.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    residual = build_residual_source()
    cases = {
        "tail-return0": residual + "\nfn tail() {\n    return 0\n}\n",
        "tail-expr-call": residual + "\nfn tail() {\n    ready()\n}\n",
        "tail-spawn-empty": residual + "\nfn tail() {\n    spawn {}\n}\n",
        "tail-return-call": residual + "\nfn tail() {\n    return ready()\n}\n",
        "tail-let-literal": residual + "\nfn tail() {\n    let x = 0\n}\n",
    }

    frames = {label: gdb_parser_frames(stage1, source) for label, source in cases.items()}
    print(f"stage1: {stage1}")
    for label in cases:
        print(f"{label}: " + " -> ".join(frames[label][:14]))

    oom_group = ["tail-return0", "tail-expr-call", "tail-spawn-empty"]
    oom_ok = all(
        "parse_expr_stmt_or_assignment" in frames[label]
        and "parse_stmt" in frames[label]
        and "parse_block" in frames[label]
        and "parse_return_stmt" not in frames[label]
        and "parse_spawn_stmt" not in frames[label]
        and "parse_let_stmt" not in frames[label]
        for label in oom_group
    )
    segv_group = ["tail-return-call", "tail-let-literal"]
    segv_ok = all(
        "parse_block" in frames[label]
        and "parse_fn_def" in frames[label]
        and "parse_item" in frames[label]
        and "parser_parse" in frames[label]
        and "parse_stmt" not in frames[label]
        and "parse_expr_stmt_or_assignment" not in frames[label]
        for label in segv_group
    )

    if oom_ok and segv_ok:
        print(
            "summary: the later-tail OOM variants unexpectedly funnel through `parse_expr_stmt_or_assignment` regardless of source spelling (`return 0`, bare `ready()`, `spawn {}`), "
            "while later-tail SIGSEGV variants like `return ready()` and `let x = 0` die earlier in `parse_block -> parse_fn_def -> parse_item -> parser_parse` without ever reaching `parse_stmt`; "
            "this strongly suggests statement dispatch is already corrupted before the OOM branch begins"
        )
        return 1

    print("summary: spawn-doc tail dispatch pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
