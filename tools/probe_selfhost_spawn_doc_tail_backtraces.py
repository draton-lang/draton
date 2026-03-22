#!/usr/bin/env python3
"""Compare parser-side backtraces for residual spawn-doc tail-function variants."""

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


def gdb_backtrace(stage1: Path, source: str) -> str:
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
        return ((result.stdout or "") + ("\n" + result.stderr if result.stderr else "")).strip()
    finally:
        path.unlink(missing_ok=True)


def extract_frames(output: str) -> list[str]:
    frames: list[str] = []
    for line in output.splitlines():
        match = FRAME_RE.match(line.strip())
        if match:
            frames.append(match.group(1))
    return frames


def parser_frames(output: str) -> list[str]:
    return [frame for frame in extract_frames(output) if frame.startswith("parser_") or frame.startswith("parse_")]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    residual = build_residual_source()
    cases = {
        "base-residual": residual,
        "tail-return0": residual + "\nfn tail() {\n    return 0\n}\n",
        "tail-expr-call": residual + "\nfn tail() {\n    ready()\n}\n",
        "tail-spawn-empty": residual + "\nfn tail() {\n    spawn {}\n}\n",
        "tail-return-call": residual + "\nfn tail() {\n    return ready()\n}\n",
    }

    outputs = {label: gdb_backtrace(stage1, source) for label, source in cases.items()}
    parser_only = {label: parser_frames(output) for label, output in outputs.items()}

    print(f"stage1: {stage1}")
    for label in cases:
        print(f"{label}: " + " -> ".join(parser_only[label][:12]))

    oom_group = ["tail-return0", "tail-expr-call", "tail-spawn-empty"]
    oom_ok = all(
        "parser_record_error" in parser_only[label]
        and "parser_error_unexpected" in parser_only[label]
        and "parse_primary" in parser_only[label]
        and "parse_postfix" in parser_only[label]
        and "parse_expr_stmt_or_assignment" in parser_only[label]
        for label in oom_group
    )
    segv_tail = parser_only["tail-return-call"]
    base = parser_only["base-residual"]
    segv_ok = (
        "parser_current" in segv_tail
        and "parser_is_eof" in segv_tail
        and "parse_block" in segv_tail
        and "parse_fn_def" in segv_tail
        and "parser_parse" in segv_tail
        and "parser_record_error" not in segv_tail
        and "parser_current" in base
        and "parser_is_eof" in base
        and "parser_parse" in base
        and "parse_block" not in base
    )

    if oom_ok and segv_ok:
        print(
            "summary: the tail-function OOM variants share a deeper parser sink "
            "(`parser_record_error -> parser_error_unexpected -> parse_primary -> parse_postfix -> parse_expr_stmt_or_assignment`), "
            "while a non-OOM tail-function variant still dies earlier at `parser_current -> parser_current_kind -> parser_is_eof -> parse_block -> parse_fn_def -> parser_parse`; "
            "the bare residual case dies even earlier at `parser_current -> parser_current_kind -> parser_is_eof -> parser_parse`"
        )
        return 1

    print("summary: spawn-doc tail backtrace pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
