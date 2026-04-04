#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import sys
from pathlib import Path


SIGNALS = {
    "term": signal.SIGTERM,
    "kill": signal.SIGKILL,
    "int": signal.SIGINT,
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Stop repository-related processes by cwd substring match.")
    parser.add_argument("--cwd", default=".")
    parser.add_argument("--contains", action="append", default=[], help="Extra substring filters.")
    parser.add_argument("--signal", choices=sorted(SIGNALS), default="term")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--exclude-pid", action="append", type=int, default=[])
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    cwd = str(Path(args.cwd).resolve())
    filters = [cwd, *args.contains]
    current_pid = os.getpid()
    parent_pid = os.getppid()
    excluded = {current_pid, parent_pid, *args.exclude_pid}
    output = subprocess.check_output(
        ["ps", "-eo", "pid=,command="],
        text=True,
    )
    signal_value = SIGNALS[args.signal]
    acted = []
    for line in output.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) != 2:
            continue
        pid_text, command = parts
        pid = int(pid_text)
        if pid in excluded:
            continue
        if not any(token and token in command for token in filters):
            continue
        entry = {"pid": pid, "signal": args.signal, "command": command}
        acted.append(entry)
        if not args.dry_run:
            try:
                os.kill(pid, signal_value)
            except ProcessLookupError:
                entry["result"] = "missing"
            except PermissionError:
                entry["result"] = "permission-denied"
            else:
                entry["result"] = "signaled"
    print(json.dumps({"cwd": cwd, "dry_run": args.dry_run, "matches": acted}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
