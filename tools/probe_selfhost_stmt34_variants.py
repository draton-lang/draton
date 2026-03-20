#!/usr/bin/env python3
"""Probe which statement-3/4 branch shapes preserve the `parser_main_prefix4` crash."""

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
VARIANTS = {
    "stmt3": [
        ("stmt3-delete", ""),
        ("stmt3-cond-ready-return-nested", "    if ready() {\n        return cmd_build(collect_cli_args(2))\n    }"),
        ("stmt3-cond-orig-return0", '    if str_eq_main(cmd, "build") {\n        return 0\n    }'),
        ("stmt3-cond-orig-return-ident", '    if str_eq_main(cmd, "build") {\n        return cmd\n    }'),
        ("stmt3-cond-orig-print", '    if str_eq_main(cmd, "build") {\n        print_usage()\n    }'),
        ("stmt3-cond-binary-return-simple-call", "    if 1 < 2 {\n        return collect_cli_args(2)\n    }"),
        ("stmt3-orig", '    if str_eq_main(cmd, "build") {\n        return cmd_build(collect_cli_args(2))\n    }'),
        ("stmt3-cond-orig-return-simple-call", '    if str_eq_main(cmd, "build") {\n        return collect_cli_args(2)\n    }'),
        ("stmt3-cond-orig-return-build-literal", '    if str_eq_main(cmd, "build") {\n        return cmd_build(2)\n    }'),
        ("stmt3-cond-orig-return-nested-other", '    if str_eq_main(cmd, "build") {\n        return cli_arg(cli_argc())\n    }'),
        ("stmt3-cond-binary-return-nested", "    if 1 < 2 {\n        return cmd_build(collect_cli_args(2))\n    }"),
        ("stmt3-cond-binary-return-grouped-simple", "    if 1 < 2 {\n        return (collect_cli_args(2))\n    }"),
    ],
    "stmt4": [
        ("stmt4-delete", ""),
        ("stmt4-cond-ready-return-nested", "    if ready() {\n        return cmd_run(collect_cli_args(2))\n    }"),
        ("stmt4-cond-orig-return0", '    if str_eq_main(cmd, "run") {\n        return 0\n    }'),
        ("stmt4-cond-orig-return-ident", '    if str_eq_main(cmd, "run") {\n        return cmd\n    }'),
        ("stmt4-cond-orig-print", '    if str_eq_main(cmd, "run") {\n        print_usage()\n    }'),
        ("stmt4-cond-binary-return-simple-call", "    if 1 < 2 {\n        return collect_cli_args(2)\n    }"),
        ("stmt4-orig", '    if str_eq_main(cmd, "run") {\n        return cmd_run(collect_cli_args(2))\n    }'),
        ("stmt4-cond-orig-return-simple-call", '    if str_eq_main(cmd, "run") {\n        return collect_cli_args(2)\n    }'),
        ("stmt4-cond-orig-return-run-literal", '    if str_eq_main(cmd, "run") {\n        return cmd_run(2)\n    }'),
        ("stmt4-cond-orig-return-nested-other", '    if str_eq_main(cmd, "run") {\n        return cli_arg(cli_argc())\n    }'),
        ("stmt4-cond-binary-return-nested", "    if 1 < 2 {\n        return cmd_run(collect_cli_args(2))\n    }"),
        ("stmt4-cond-binary-return-grouped-simple", "    if 1 < 2 {\n        return (collect_cli_args(2))\n    }"),
    ],
}
EXPECTED_PASSES = {
    "stmt3-delete",
    "stmt3-cond-ready-return-nested",
    "stmt3-cond-orig-return0",
    "stmt3-cond-orig-return-ident",
    "stmt3-cond-orig-print",
    "stmt3-cond-binary-return-simple-call",
    "stmt4-delete",
    "stmt4-cond-ready-return-nested",
    "stmt4-cond-orig-return0",
    "stmt4-cond-orig-return-ident",
    "stmt4-cond-orig-print",
    "stmt4-cond-binary-return-simple-call",
}
EXPECTED_FAILS = {
    "stmt3-orig",
    "stmt3-cond-orig-return-simple-call",
    "stmt3-cond-orig-return-build-literal",
    "stmt3-cond-orig-return-nested-other",
    "stmt3-cond-binary-return-nested",
    "stmt3-cond-binary-return-grouped-simple",
    "stmt4-orig",
    "stmt4-cond-orig-return-simple-call",
    "stmt4-cond-orig-return-run-literal",
    "stmt4-cond-orig-return-nested-other",
    "stmt4-cond-binary-return-nested",
    "stmt4-cond-binary-return-grouped-simple",
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
    for stmt_name, variants in VARIANTS.items():
        print(f"\n[{stmt_name}]")
        needle = NEEDLES[stmt_name]
        for label, replacement in variants:
            code = run_text(stage1, base.replace(needle, replacement))
            print(f"{label}: returncode={code}")
            if label in EXPECTED_PASSES and code != 0:
                pass_labels_pass = False
            if label in EXPECTED_FAILS and code == 0:
                fail_labels_fail = False

    if pass_labels_pass and fail_labels_fail:
        print(
            "summary: stmt3/stmt4 only preserve the crash under high-pressure condition+call-return pairs; "
            "the original two-argument condition keeps failing with call returns, and binary conditions can still "
            "fail with grouped or nested call returns"
        )
        return 1

    print("summary: stmt3/stmt4 variant pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
