#!/usr/bin/env python3
"""Probe call-arity sensitivity for grouped stmt3/stmt4 bodies."""

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
REPLACEMENTS = {
    "stmt3": [
        ("str_eq_main-2arg", '    if str_eq_main(cmd, "build") {\n        (cmd)\n    }'),
        ("foo-2arg-ident-lit", '    if foo(cmd, "build") {\n        (cmd)\n    }'),
        ("foo-2arg-ident-ident", '    if foo(cmd, cmd) {\n        (cmd)\n    }'),
        ("foo-2arg-lit-lit", '    if foo("build", "build") {\n        (cmd)\n    }'),
        ("foo-2arg-call-lit", '    if foo(cli_arg(1), "build") {\n        (cmd)\n    }'),
        ("foo-2arg-lit-call", '    if foo("build", cli_arg(1)) {\n        (cmd)\n    }'),
        ("foo-1arg", "    if foo(cmd) {\n        (cmd)\n    }"),
        ("foo-3arg", '    if foo(cmd, "build", cmd) {\n        (cmd)\n    }'),
        ("foo-0arg", "    if foo() {\n        (cmd)\n    }"),
        ("foo-nested-2arg", '    if foo(bar(cmd), baz("build")) {\n        (cmd)\n    }'),
        ("qualified-2arg", '    if math.eq(cmd, "build") {\n        (cmd)\n    }'),
        ("qualified-1arg", "    if math.eq(cmd) {\n        (cmd)\n    }"),
    ],
    "stmt4": [
        ("foo-2arg", '    if foo(cmd, "run") {\n        (cmd)\n    }'),
        ("foo-1arg", "    if foo(cmd) {\n        (cmd)\n    }"),
        ("foo-3arg", '    if foo(cmd, "run", cmd) {\n        (cmd)\n    }'),
        ("foo-0arg", "    if foo() {\n        (cmd)\n    }"),
        ("qualified-2arg", '    if math.eq(cmd, "run") {\n        (cmd)\n    }'),
        ("qualified-1arg", "    if math.eq(cmd) {\n        (cmd)\n    }"),
    ],
}
EXPECTED_PASSES = {
    "stmt3-foo-1arg",
    "stmt3-foo-0arg",
    "stmt3-qualified-1arg",
    "stmt4-foo-1arg",
    "stmt4-foo-0arg",
    "stmt4-qualified-1arg",
}
EXPECTED_FAILS = {
    "stmt3-str_eq_main-2arg",
    "stmt3-foo-2arg-ident-lit",
    "stmt3-foo-2arg-ident-ident",
    "stmt3-foo-2arg-lit-lit",
    "stmt3-foo-2arg-call-lit",
    "stmt3-foo-2arg-lit-call",
    "stmt3-foo-3arg",
    "stmt3-foo-nested-2arg",
    "stmt3-qualified-2arg",
    "stmt4-foo-2arg",
    "stmt4-foo-3arg",
    "stmt4-qualified-2arg",
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
            "summary: grouped stmt3/stmt4 bodies fail for multi-argument call-like conditions, "
            "while zero-arg and one-arg call conditions still pass"
        )
        return 1

    print("summary: stmt3/stmt4 condition-arity pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
