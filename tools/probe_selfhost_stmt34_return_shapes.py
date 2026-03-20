#!/usr/bin/env python3
"""Probe return-shape pressure inside statement-3/4 command branches."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"
NEEDLES = {
    "stmt3": '    if str_eq_main(cmd, "build") {\n        return cmd_build(collect_cli_args(2))\n    }',
    "stmt4": '    if str_eq_main(cmd, "run") {\n        return cmd_run(collect_cli_args(2))\n    }',
}
WRAPPERS = {
    "stmt3": '    if str_eq_main(cmd, "build") {{\n{body}\n    }}',
    "stmt4": '    if str_eq_main(cmd, "run") {{\n{body}\n    }}',
}
RETURN_VARIANTS = [
    ("return-ident", "        return cmd"),
    ("return-zero-arg-call", "        return cli_argc()"),
    ("return-zero-arg-call-grouped", "        return (cli_argc())"),
    ("return-one-arg-call", "        return collect_cli_args(2)"),
    ("return-one-arg-call-grouped", "        return (collect_cli_args(2))"),
    ("return-wrapper-one-arg", "        return cmd_build(2)"),
    ("return-wrapper-one-arg-grouped", "        return (cmd_build(2))"),
    ("return-nested-call", "        return cmd_build(collect_cli_args(2))"),
    ("return-nested-other", "        return cli_arg(cli_argc())"),
    ("return-nested-other-grouped", "        return (cli_arg(cli_argc()))"),
]
EXPECTED_PASSES = {
    "stmt3-return-ident",
    "stmt3-return-zero-arg-call",
    "stmt4-return-ident",
    "stmt4-return-zero-arg-call",
}
EXPECTED_FAILS = {
    "stmt3-return-zero-arg-call-grouped",
    "stmt3-return-one-arg-call",
    "stmt3-return-one-arg-call-grouped",
    "stmt3-return-wrapper-one-arg",
    "stmt3-return-wrapper-one-arg-grouped",
    "stmt3-return-nested-call",
    "stmt3-return-nested-other",
    "stmt3-return-nested-other-grouped",
    "stmt4-return-zero-arg-call-grouped",
    "stmt4-return-one-arg-call",
    "stmt4-return-one-arg-call-grouped",
    "stmt4-return-wrapper-one-arg",
    "stmt4-return-wrapper-one-arg-grouped",
    "stmt4-return-nested-call",
    "stmt4-return-nested-other",
    "stmt4-return-nested-other-grouped",
}


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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage1", default=None, help="path to a stage1 self-host binary")
    args = parser.parse_args()

    stage1 = Path(args.stage1) if args.stage1 else find_stage1()
    base = FIXTURE.read_text(encoding="utf-8")
    pass_labels_pass = True
    fail_labels_fail = True

    print(f"stage1: {stage1}")
    for stmt_name in ("stmt3", "stmt4"):
        print(f"\n[{stmt_name}]")
        needle = NEEDLES[stmt_name]
        wrapper = WRAPPERS[stmt_name]
        for label, body in RETURN_VARIANTS:
            full_label = f"{stmt_name}-{label}"
            replacement = wrapper.format(body=body)
            code = run_text(stage1, base.replace(needle, replacement))
            print(f"{full_label}: returncode={code}")
            if full_label in EXPECTED_PASSES and code != 0:
                pass_labels_pass = False
            if full_label in EXPECTED_FAILS and code == 0:
                fail_labels_fail = False

    if pass_labels_pass and fail_labels_fail:
        print(
            "summary: under the original stmt3/stmt4 conditions, only ident returns and ungrouped zero-arg calls pass; "
            "grouped zero-arg calls, one-arg calls, wrapper calls, and nested calls all preserve the crash"
        )
        return 1

    print("summary: stmt3/stmt4 return-shape pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
