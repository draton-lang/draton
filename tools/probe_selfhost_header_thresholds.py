#!/usr/bin/env python3
"""Probe payload thresholds for the `parser_main_prefix4` header sections."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"


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


def run_text(stage1: Path, text: str) -> int:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".dt", delete=False) as handle:
        handle.write(text)
        path = Path(handle.name)
    try:
        result = subprocess.run(
            [str(stage1), "ast-dump", str(path)],
            cwd=REPO,
            capture_output=True,
        )
        return result.returncode
    finally:
        path.unlink(missing_ok=True)


def render_class(count: int) -> str:
    field_names = [
        "source_path",
        "positional",
        "flags",
        "named_keys",
        "named_values",
    ]
    field_types = [
        "String",
        "Array[String]",
        "Array[String]",
        "Array[String]",
        "Array[String]",
    ]
    lines = ["class ParsedArgs {"]
    for name in field_names[:count]:
        lines.append(f"    let mut {name}")
    lines.append("")
    lines.append("    @type {")
    for name, ty in zip(field_names[:count], field_types[:count]):
        lines.append(f"        {name}: {ty}")
    lines.append("    }")
    lines.append("}")
    return "\n".join(lines) + "\n\n"


def render_type_block(count: int) -> str:
    entries = [
        "main: () -> Int",
        "cmd_build: (Array[String]) -> Int",
        "cmd_run: (Array[String]) -> Int",
        "cmd_check: (Array[String]) -> Int",
        "cmd_ast_dump: (String) -> Int",
        "cmd_type_dump: (String) -> Int",
        "parse_args: (Array[String]) -> ParsedArgs",
        "parsed_args_has_flag: (ParsedArgs, String) -> Bool",
        "parsed_args_flag_value: (ParsedArgs, String) -> String",
        "parsed_args_trailing_args: (ParsedArgs) -> Array[String]",
        "collect_cli_args: (Int) -> Array[String]",
        "print_pipeline_error: (PipelineError) -> Unit",
        "print_usage: () -> Unit",
        "join_lex_errors: (Array[LexError]) -> String",
        "join_parse_errors: (Array[ParseError]) -> String",
        "join_type_errors: (Array[TypeError]) -> String",
        "str_eq_main: (String, String) -> Bool",
        "str_starts_with_main: (String, String) -> Bool",
        "arg_takes_value: (String) -> Bool",
        "cli_argc: () -> Int",
        "cli_arg: (Int) -> String",
        "read_file: (String) -> String",
    ]
    lines = ["@type {"]
    for entry in entries[:count]:
        lines.append(f"    {entry}")
    lines.append("}")
    return "\n".join(lines) + "\n\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    lines = FIXTURE.read_text(encoding="utf-8").splitlines()
    imports = "\n".join(lines[0:5]) + "\n\n"
    main_block = "\n".join(lines[47:]) + "\n"

    print(f"stage1: {stage1}")
    print("class-thresholds:")
    first_class_failure = -1
    for count in range(0, 6):
        code = run_text(stage1, imports + render_class(count) + render_type_block(22) + main_block)
        print(f"class-fields-{count}: returncode={code}")
        if code != 0 and first_class_failure < 0:
            first_class_failure = count

    print("type-thresholds:")
    first_type_failure = -1
    for count in range(0, 23):
        code = run_text(stage1, imports + render_class(5) + render_type_block(count) + main_block)
        print(f"type-entries-{count}: returncode={code}")
        if code != 0 and first_type_failure < 0:
            first_type_failure = count

    print(f"first failing class field count: {first_class_failure}")
    print(f"first failing type entry count: {first_type_failure}")
    return 1 if first_class_failure >= 0 or first_type_failure >= 0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
