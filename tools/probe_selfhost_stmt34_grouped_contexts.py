#!/usr/bin/env python3
"""Probe grouped-expression positions and condition sensitivity in stmt3/stmt4."""

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
ORIGINAL_WRAPPERS = {
    "stmt3": '    if str_eq_main(cmd, "build") {{\n{body}\n    }}',
    "stmt4": '    if str_eq_main(cmd, "run") {{\n{body}\n    }}',
}
POSITION_VARIANTS = [
    ("return-ident", "        return cmd"),
    ("return-grouped-ident", "        return (cmd)"),
    ("let-ident", "        let warm = cmd"),
    ("let-grouped-ident", "        let warm = (cmd)"),
    ("expr-ident", "        cmd"),
    ("expr-grouped-ident", "        (cmd)"),
    ("return-literal", "        return 0"),
    ("return-grouped-literal", "        return (0)"),
    ("let-grouped-literal", "        let warm = (0)"),
    ("expr-grouped-literal", "        (0)"),
    ("return-zero-arg-call", "        return cli_argc()"),
    ("return-grouped-zero-arg-call", "        return (cli_argc())"),
    ("let-grouped-zero-arg-call", "        let warm = (cli_argc())"),
    ("expr-grouped-zero-arg-call", "        (cli_argc())"),
]
CONDITION_VARIANTS = [
    ("orig-cond-expr-grouped-ident", '    if str_eq_main(cmd, "build") {\n        (cmd)\n    }'),
    ("orig-cond-let-grouped-ident", '    if str_eq_main(cmd, "build") {\n        let warm = (cmd)\n    }'),
    ("ready-cond-expr-grouped-ident", "    if ready() {\n        (cmd)\n    }"),
    ("ready-cond-let-grouped-ident", "    if ready() {\n        let warm = (cmd)\n    }"),
    ("cliargc-cond-expr-grouped-ident", "    if cli_argc() {\n        (cmd)\n    }"),
    ("binary-cond-expr-grouped-ident", "    if 1 < 2 {\n        (cmd)\n    }"),
]
EXPECTED_PASSES = {
    "stmt3-return-ident",
    "stmt3-let-ident",
    "stmt3-expr-ident",
    "stmt3-return-literal",
    "stmt3-return-zero-arg-call",
    "stmt4-return-ident",
    "stmt4-let-ident",
    "stmt4-expr-ident",
    "stmt4-return-literal",
    "stmt4-return-zero-arg-call",
    "stmt3-ready-cond-expr-grouped-ident",
    "stmt3-ready-cond-let-grouped-ident",
    "stmt3-cliargc-cond-expr-grouped-ident",
    "stmt3-binary-cond-expr-grouped-ident",
}
EXPECTED_FAILS = {
    "stmt3-return-grouped-ident",
    "stmt3-let-grouped-ident",
    "stmt3-expr-grouped-ident",
    "stmt3-return-grouped-literal",
    "stmt3-let-grouped-literal",
    "stmt3-expr-grouped-literal",
    "stmt3-return-grouped-zero-arg-call",
    "stmt3-let-grouped-zero-arg-call",
    "stmt3-expr-grouped-zero-arg-call",
    "stmt4-return-grouped-ident",
    "stmt4-let-grouped-ident",
    "stmt4-expr-grouped-ident",
    "stmt4-return-grouped-literal",
    "stmt4-let-grouped-literal",
    "stmt4-expr-grouped-literal",
    "stmt4-return-grouped-zero-arg-call",
    "stmt4-let-grouped-zero-arg-call",
    "stmt4-expr-grouped-zero-arg-call",
    "stmt3-orig-cond-expr-grouped-ident",
    "stmt3-orig-cond-let-grouped-ident",
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
        print(f"\n[{stmt_name}: grouped positions under original condition]")
        needle = NEEDLES[stmt_name]
        wrapper = ORIGINAL_WRAPPERS[stmt_name]
        for label, body in POSITION_VARIANTS:
            full_label = f"{stmt_name}-{label}"
            replacement = wrapper.format(body=body)
            code = run_text(stage1, base.replace(needle, replacement))
            print(f"{full_label}: returncode={code}")
            if full_label in EXPECTED_PASSES and code != 0:
                pass_labels_pass = False
            if full_label in EXPECTED_FAILS and code == 0:
                fail_labels_fail = False

    print("\n[stmt3: grouped-expression condition sensitivity]")
    for label, replacement in CONDITION_VARIANTS:
        full_label = f"stmt3-{label}"
        code = run_text(stage1, base.replace(NEEDLES["stmt3"], replacement))
        print(f"{full_label}: returncode={code}")
        if full_label in EXPECTED_PASSES and code != 0:
            pass_labels_pass = False
        if full_label in EXPECTED_FAILS and code == 0:
            fail_labels_fail = False

    if pass_labels_pass and fail_labels_fail:
        print(
            "summary: under the original stmt3/stmt4 condition, grouped expressions fail in return, let, and bare-expression positions; "
            "the same grouped expressions pass again once the condition is simplified away from str_eq_main(cmd, ...)"
        )
        return 1

    print("summary: stmt3/stmt4 grouped-context pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
