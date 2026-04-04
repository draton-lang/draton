#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path


def read_text(path: str) -> str | None:
    try:
        return Path(path).read_text(encoding="utf-8").strip()
    except OSError:
        return None


def capture(command: list[str]) -> str | None:
    try:
        return subprocess.check_output(command, text=True, stderr=subprocess.DEVNULL).strip()
    except Exception:
        return None


def main() -> int:
    loadavg = os.getloadavg() if hasattr(os, "getloadavg") else None
    meminfo = {}
    meminfo_raw = read_text("/proc/meminfo")
    if meminfo_raw:
        for line in meminfo_raw.splitlines():
            key, value = line.split(":", 1)
            meminfo[key] = value.strip()
    data = {
        "timestamp": int(time.time()),
        "hostname": capture(["hostname"]),
        "cwd": str(Path.cwd()),
        "python": sys.executable,
        "cpu_count": os.cpu_count(),
        "loadavg": loadavg,
        "meminfo": {
            "MemTotal": meminfo.get("MemTotal"),
            "MemAvailable": meminfo.get("MemAvailable"),
            "SwapTotal": meminfo.get("SwapTotal"),
            "SwapFree": meminfo.get("SwapFree"),
        },
        "disk_free": shutil.disk_usage(Path.cwd())._asdict(),
        "uptime": read_text("/proc/uptime"),
    }
    print(json.dumps(data, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
