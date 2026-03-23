#!/usr/bin/env python3
"""Probe header payload thresholds for a representative residual spawn-doc tail-body pair."""

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
TAIL = "fn tail() {\n    spawn {}\n    return ready()\n}\n"


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


def rewrite_header(source: str, class_count: int, type_count: int) -> str:
    class_marker = "class ParsedArgs {"
    class_type_marker = "    @type {"
    class_start = source.index(class_marker)
    class_type_idx = source.index(class_type_marker, class_start)
    class_fields_block = source[source.index("\n", class_start) + 1 : class_type_idx]
    class_fields = [line for line in class_fields_block.splitlines() if line.strip().startswith("let mut ")]

    type_start = source.index("@type {\n    main: () -> Int")
    main_start = source.index("fn main() {")
    type_block = source[type_start:main_start]
    type_entries = [line for line in type_block.splitlines()[1:-1] if line.strip()]

    new_fields = "\n".join(class_fields[:class_count]) + ("\n\n" if class_count else "\n")
    rewritten = source[: source.index("\n", class_start) + 1] + new_fields + source[class_type_idx:]
    type_start2 = rewritten.index("@type {\n    main: () -> Int")
    main_start2 = rewritten.index("fn main() {")
    new_type = "@type {\n" + "\n".join(type_entries[:type_count]) + "\n}\n\n"
    return rewritten[:type_start2] + new_type + rewritten[main_start2:]


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

    print(f"stage1: {stage1}")
    class_results: dict[int, tuple[int, str]] = {}
    print("class sweep @ type22")
    for class_count in range(0, 6):
        code, msg = run_text(stage1, rewrite_header(residual, class_count, 22) + "\n" + TAIL)
        class_results[class_count] = (code, msg)
        print(f"class-{class_count}: returncode={code}")
        if msg:
            print(f"class-{class_count}: message={msg}")

    type_results: dict[int, tuple[int, str]] = {}
    type_counts = [0, 8, 12, 15, 16, 18, 20, 22]
    print("type sweep @ class5")
    for type_count in type_counts:
        code, msg = run_text(stage1, rewrite_header(residual, 5, type_count) + "\n" + TAIL)
        type_results[type_count] = (code, msg)
        print(f"type-{type_count}: returncode={code}")
        if msg:
            print(f"type-{type_count}: message={msg}")

    if (
        all(class_results[count][0] == -11 for count in [0, 1, 2, 3, 4])
        and class_results[5][0] == -6
        and "memory allocation" in class_results[5][1]
        and all(type_results[count][0] == 0 for count in [0, 8, 12])
        and all(type_results[count][0] == -11 for count in [15, 16, 18, 20])
        and type_results[22][0] == -6
        and "memory allocation" in type_results[22][1]
    ):
        print(
            "summary: for the representative residual tail-body pair `spawn {} + return ready()`, header pressure splits into distinct regions: "
            "full class payload is required before the pair reaches the OOM branch, while type payload has three phases at class5: low counts pass, mid counts SIGSEGV, and only the full 22-entry block reaches OOM"
        )
        return 1

    print("summary: spawn-doc header-threshold pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
