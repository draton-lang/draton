#!/usr/bin/env python3
"""Probe condition-shape sensitivity for grouped bodies in stmt3/stmt4."""

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
        ("orig", '    if str_eq_main(cmd, "build") {\n        (cmd)\n    }'),
        ("strcmp-self", '    if str_eq_main(cmd, cmd) {\n        (cmd)\n    }'),
        ("strcmp-lit-lit", '    if str_eq_main("build", "build") {\n        (cmd)\n    }'),
        ("strcmp-call-lit", '    if str_eq_main(cli_arg(1), "build") {\n        (cmd)\n    }'),
        ("strcmp-lit-call", '    if str_eq_main("build", cli_arg(1)) {\n        (cmd)\n    }'),
        ("ready", "    if ready() {\n        (cmd)\n    }"),
        ("cliargc", "    if cli_argc() {\n        (cmd)\n    }"),
        ("one-arg-call", "    if collect_cli_args(2) {\n        (cmd)\n    }"),
        ("cli-arg", "    if cli_arg(1) {\n        (cmd)\n    }"),
        ("binary", "    if 1 < 2 {\n        (cmd)\n    }"),
        ("binary-call", "    if cli_argc() < 2 {\n        (cmd)\n    }"),
        ("grouped-orig-cond", '    if (str_eq_main(cmd, "build")) {\n        (cmd)\n    }'),
        ("grouped-ready-cond", "    if (ready()) {\n        (cmd)\n    }"),
    ],
    "stmt4": [
        ("orig", '    if str_eq_main(cmd, "run") {\n        (cmd)\n    }'),
        ("strcmp-self", '    if str_eq_main(cmd, cmd) {\n        (cmd)\n    }'),
        ("strcmp-lit-lit", '    if str_eq_main("run", "run") {\n        (cmd)\n    }'),
        ("ready", "    if ready() {\n        (cmd)\n    }"),
        ("one-arg-call", "    if collect_cli_args(2) {\n        (cmd)\n    }"),
        ("binary", "    if 1 < 2 {\n        (cmd)\n    }"),
        ("grouped-orig-cond", '    if (str_eq_main(cmd, "run")) {\n        (cmd)\n    }'),
        ("grouped-ready-cond", "    if (ready()) {\n        (cmd)\n    }"),
    ],
}
EXPECTED_PASSES = {
    "stmt3-ready",
    "stmt3-cliargc",
    "stmt3-one-arg-call",
    "stmt3-cli-arg",
    "stmt3-binary",
    "stmt3-binary-call",
    "stmt3-grouped-ready-cond",
    "stmt4-ready",
    "stmt4-one-arg-call",
    "stmt4-binary",
    "stmt4-grouped-ready-cond",
}
EXPECTED_FAILS = {
    "stmt3-orig",
    "stmt3-strcmp-self",
    "stmt3-strcmp-lit-lit",
    "stmt3-strcmp-call-lit",
    "stmt3-strcmp-lit-call",
    "stmt3-grouped-orig-cond",
    "stmt4-orig",
    "stmt4-strcmp-self",
    "stmt4-strcmp-lit-lit",
    "stmt4-grouped-orig-cond",
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
            "summary: grouped stmt3/stmt4 bodies only crash under str_eq_main-style conditions; "
            "simpler call and binary conditions still pass even when the body stays grouped"
        )
        return 1

    print("summary: stmt3/stmt4 condition-shape pattern changed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
