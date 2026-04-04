#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="List processes that appear related to the current repository path.")
    parser.add_argument("--cwd", default=".")
    parser.add_argument("--contains", action="append", default=[], help="Extra substring filters.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    cwd = str(Path(args.cwd).resolve())
    output = subprocess.check_output(
        ["ps", "-eo", "pid=,ppid=,etimes=,%cpu=,%mem=,command="],
        text=True,
    )
    matches = []
    filters = [cwd, *args.contains]
    for line in output.splitlines():
        parts = line.strip().split(None, 5)
        if len(parts) != 6:
            continue
        pid, ppid, etimes, cpu, mem, command = parts
        if any(token and token in command for token in filters):
            matches.append(
                {
                    "pid": int(pid),
                    "ppid": int(ppid),
                    "elapsed_sec": int(etimes),
                    "cpu_percent": float(cpu),
                    "mem_percent": float(mem),
                    "command": command,
                }
            )
    print(json.dumps({"cwd": cwd, "matches": matches}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
