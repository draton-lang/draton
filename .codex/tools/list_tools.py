#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent


def main() -> int:
    data = {
        "tools_dir": str(SCRIPT_DIR),
        "recommended_entrypoint": str(SCRIPT_DIR / "run_guarded.py"),
        "tools": [
            {
                "name": "run_guarded.py",
                "purpose": "Run commands with timeout, memory, CPU, file-size, and concurrency limits.",
                "best_for": [
                    "cargo test",
                    "cargo build",
                    "release smoke tests",
                    "long-running repository scripts",
                ],
            },
            {
                "name": "guarded_cargo.py",
                "purpose": "Run common cargo workflows through guarded presets.",
                "best_for": [
                    "parser tests",
                    "typechecker tests",
                    "workspace builds",
                    "workspace clippy or fmt checks",
                ],
            },
            {
                "name": "system_snapshot.py",
                "purpose": "Capture host load, memory, and disk state before expensive work.",
                "best_for": [
                    "resource triage",
                    "preflight checks",
                ],
            },
            {
                "name": "repo_processes.py",
                "purpose": "List repository-related processes and their resource usage.",
                "best_for": [
                    "stuck builds",
                    "overlapping test runs",
                    "diagnostics",
                ],
            },
            {
                "name": "stop_repo_processes.py",
                "purpose": "Stop repository-related processes with controlled signals.",
                "best_for": [
                    "clean shutdown",
                    "stuck repo jobs",
                ],
            },
            {
                "name": "cleanup_tool_state.py",
                "purpose": "Clear stale guarded-run slot records when interrupted runs leave state behind.",
                "best_for": [
                    "interrupted sessions",
                    "stale slot cleanup",
                ],
            },
            {
                "name": "list_tools.py",
                "purpose": "Describe the local Codex tooling set.",
                "best_for": [
                    "tool discovery",
                    "skill references",
                ],
            },
        ],
        "defaults": {
            "timeout_sec": 900,
            "wait_sec": 120,
            "concurrency": 2,
            "memory_mb": 2048,
            "cpu_seconds": 600,
        },
    }
    print(json.dumps(data, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
