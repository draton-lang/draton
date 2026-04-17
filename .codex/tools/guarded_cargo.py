#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
RUN_GUARDED = SCRIPT_DIR / "run_guarded.py"

PRESETS = {
    "parser": {"timeout": 900, "memory": 2048, "cpu": 600, "args": ["test", "-p", "draton-parser", "--test", "items"]},
    "typeck": {"timeout": 900, "memory": 2048, "cpu": 600, "args": ["test", "-p", "draton-typeck", "--test", "errors"]},
    "selfhost-stage0": {
        "timeout": 1800,
        "memory": 4096,
        "cpu": 1200,
        "file_size": 2048,
        "args": ["test", "-p", "drat", "--test", "selfhost_stage0", "--", "--nocapture"],
    },
    "workspace-test": {"timeout": 1800, "memory": 4096, "cpu": 1200, "args": ["test", "--workspace"]},
    "workspace-build": {"timeout": 1800, "memory": 4096, "cpu": 1200, "args": ["build", "--workspace"]},
    "workspace-clippy": {"timeout": 1800, "memory": 4096, "cpu": 1200, "args": ["clippy", "--workspace", "--", "-D", "warnings"]},
    "workspace-fmt-check": {"timeout": 600, "memory": 1024, "cpu": 300, "args": ["fmt", "--all", "--check"]},
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run common cargo workflows through run_guarded.py presets.")
    parser.add_argument("--cwd", default=".")
    parser.add_argument("--preset", choices=sorted(PRESETS))
    parser.add_argument("--timeout-sec", type=int)
    parser.add_argument("--memory-mb", type=int)
    parser.add_argument("--cpu-seconds", type=int)
    parser.add_argument("--file-size-mb", type=int)
    parser.add_argument("--concurrency", type=int, default=2)
    parser.add_argument("--wait-sec", type=int, default=120)
    parser.add_argument("--json-only", action="store_true")
    parser.add_argument("cargo_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    while args.cargo_args and args.cargo_args[0] == "--":
        args.cargo_args = args.cargo_args[1:]
    if args.preset is None and not args.cargo_args:
        parser.error("provide --preset or cargo args")
    return args


def resolve_cargo() -> str:
    explicit = os.environ.get("CARGO")
    if explicit:
        candidate = Path(explicit).expanduser()
        if candidate.exists():
            return str(candidate)
    candidates = [
        Path.home() / ".cargo" / "bin" / "cargo",
        Path.home() / ".cargo" / "bin" / "cargo.exe",
        Path("/home/lehungquangminh/.cargo/bin/cargo"),
    ]
    for candidate in candidates:
        if candidate.exists():
            return str(candidate)
    found = shutil.which("cargo")
    if found:
        return found
    found = shutil.which("cargo.exe")
    if found:
        return found
    raise SystemExit("cargo executable not found; set CARGO or install cargo in PATH")


def preset_env(preset_name: str | None, cwd: str) -> list[tuple[str, str]]:
    if preset_name != "selfhost-stage0":
        return []

    workspace = Path(cwd).resolve()
    repo_name = workspace.name or "repo"
    repo_hash = hashlib.sha256(str(workspace).encode("utf-8")).hexdigest()[:12]
    root = Path(tempfile.gettempdir()) / "draton-guarded" / f"{repo_name}-{repo_hash}" / preset_name
    target_dir = root / "target"
    tmp_dir = root / "tmp"
    target_dir.mkdir(parents=True, exist_ok=True)
    tmp_dir.mkdir(parents=True, exist_ok=True)
    return [
        ("CARGO_TARGET_DIR", str(target_dir)),
        ("TMPDIR", str(tmp_dir)),
        ("TMP", str(tmp_dir)),
        ("TEMP", str(tmp_dir)),
    ]


def main() -> int:
    args = parse_args()
    cwd = str(Path(args.cwd).resolve())
    if args.preset is not None:
        preset = PRESETS[args.preset]
        cargo_args = preset["args"]
        timeout = args.timeout_sec or preset["timeout"]
        memory = args.memory_mb or preset["memory"]
        cpu = args.cpu_seconds or preset["cpu"]
        file_size = args.file_size_mb or preset.get("file_size", 64)
    else:
        cargo_args = args.cargo_args
        timeout = args.timeout_sec or 1200
        memory = args.memory_mb or 3072
        cpu = args.cpu_seconds or 900
        file_size = args.file_size_mb or 64
    command = [
        sys.executable,
        str(RUN_GUARDED),
        "--cwd",
        cwd,
        "--timeout-sec",
        str(timeout),
        "--memory-mb",
        str(memory),
        "--cpu-seconds",
        str(cpu),
        "--file-size-mb",
        str(file_size),
        "--concurrency",
        str(args.concurrency),
        "--wait-sec",
        str(args.wait_sec),
    ]
    for key, value in preset_env(args.preset, cwd):
        command.extend(["--env", f"{key}={value}"])
    if args.json_only:
        command.append("--json-only")
    command.extend(["--", resolve_cargo(), *cargo_args])
    return subprocess.call(command)


if __name__ == "__main__":
    raise SystemExit(main())
