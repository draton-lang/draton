#!/usr/bin/env python3
"""Compare the main self-host parser crash stack with the residual spawn-doc stack."""

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
FRAME_RE = re.compile(r"^#\d+\s+(?:0x[0-9a-f]+\s+in\s+)?([A-Za-z0-9_]+)")


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


def gdb_backtrace(stage1: Path, input_path: Path) -> tuple[int, str]:
    argv = [
        "gdb",
        "-q",
        "-batch",
        "-ex",
        "set pagination off",
        "-ex",
        f"run ast-dump {input_path}",
        "-ex",
        "bt",
        "--args",
        str(stage1),
    ]
    result = subprocess.run(argv, cwd=REPO, capture_output=True, text=True)
    output = (result.stdout or "") + ("\n" + result.stderr if result.stderr else "")
    return result.returncode, output.strip()


def extract_frames(output: str) -> list[str]:
    frames: list[str] = []
    for line in output.splitlines():
        match = FRAME_RE.match(line.strip())
        if match:
            frames.append(match.group(1))
    return frames


def build_spawn_doc_variant() -> str:
    base = FIXTURE.read_text(encoding="utf-8")
    return base.replace(STMT3 + "\n" + STMT4, BAD3 + "\n" + SPAWN_DOC + "\n" + BAD4)


def run_variant(stage1: Path, source: str) -> tuple[int, str]:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".dt", delete=False) as handle:
        handle.write(source)
        path = Path(handle.name)
    try:
        return gdb_backtrace(stage1, path)
    finally:
        path.unlink(missing_ok=True)


def format_frames(label: str, frames: list[str], limit: int = 8) -> str:
    shown = frames[:limit]
    return f"{label}: " + " -> ".join(shown)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    baseline_code, baseline_output = gdb_backtrace(stage1, FIXTURE)
    residual_code, residual_output = run_variant(stage1, build_spawn_doc_variant())
    baseline_frames = extract_frames(baseline_output)
    residual_frames = extract_frames(residual_output)

    print(f"stage1: {stage1}")
    print(f"baseline-returncode: {baseline_code}")
    print(format_frames("baseline-frames", baseline_frames))
    print(f"residual-returncode: {residual_code}")
    print(format_frames("residual-frames", residual_frames))

    if (
        baseline_code == 0
        and residual_code == 0
        and "parse_return_stmt" in baseline_frames
        and "parse_arg_list" in baseline_frames
        and "parse_postfix" in baseline_frames
        and "parser_parse" in residual_frames
        and "parse_return_stmt" not in residual_frames
        and "parse_arg_list" not in residual_frames
        and "parse_postfix" not in residual_frames
    ):
        print(
            "summary: the residual both-bad + spawn-doc crash follows a shallower stack "
            "(`parser_current -> parser_current_kind -> parser_is_eof -> parser_parse`) than the main prefix-4 crash, "
            "which still runs through `parse_return_stmt -> parse_arg_list -> parse_postfix`; treat them as distinct crash paths until proven otherwise"
        )
        return 1

    print("summary: spawn-doc backtrace pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
