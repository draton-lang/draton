#!/usr/bin/env python3
"""Probe comma-bearing condition expressions for grouped stmt3/stmt4 bodies."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "tests" / "programs" / "selfhost" / "parser_main_prefix4.dt"
NEEDLES = {
    "stmt3": '    if str_eq_main(cmd, "build") {\n        return cmd_build(collect_cli_args(2))\n    }',
}
REPLACEMENTS = {
    "stmt3": [
        ("tuple-cond", '    if (cmd, "build") {\n        (cmd)\n    }'),
        ("array-cond", '    if [cmd, "build"] {\n        (cmd)\n    }'),
        ("brace-cond", '    if { left: cmd, right: "build" } {\n        (cmd)\n    }'),
        ("index-2arg", '    if foo[cmd, "build"] {\n        (cmd)\n    }'),
        ("index-1arg", '    if foo[cmd] {\n        (cmd)\n    }'),
        ("grouped-array-cond", '    if ([cmd, "build"]) {\n        (cmd)\n    }'),
        ("call-array-1arg", '    if foo([cmd, "build"]) {\n        (cmd)\n    }'),
        ("call-tuple-1arg", '    if foo((cmd, "build")) {\n        (cmd)\n    }'),
    ],
}
EXPECTED_PASSES = {
    "stmt3-index-1arg",
}
EXPECTED_FAILS = {
    "stmt3-tuple-cond",
    "stmt3-array-cond",
    "stmt3-brace-cond",
    "stmt3-index-2arg",
    "stmt3-grouped-array-cond",
    "stmt3-call-array-1arg",
    "stmt3-call-tuple-1arg",
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
    for stmt_name, replacements in REPLACEMENTS.items():
        print(f"\n[{stmt_name}]")
        needle = NEEDLES[stmt_name]
        for label, replacement in replacements:
            full_label = f"{stmt_name}-{label}"
            code = run_text(stage1, base.replace(needle, replacement))
            print(f"{full_label}: returncode={code}")
            if full_label in EXPECTED_PASSES and code != 0:
                pass_labels_pass = False
            if full_label in EXPECTED_FAILS and code == 0:
                fail_labels_fail = False

    if pass_labels_pass and fail_labels_fail:
        print(
            "summary: grouped stmt3/stmt4 bodies fail for comma-bearing condition expressions more generally, "
            "not just for multi-argument call syntax; single-index conditions still pass"
        )
        return 1

    print("summary: stmt3/stmt4 comma-bearing condition pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
