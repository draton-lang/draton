#!/usr/bin/env python3
from __future__ import annotations

import fcntl
import json
import os
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
STATE_DIR = SCRIPT_DIR / ".state"
SLOTS_PATH = STATE_DIR / "slots.json"
LOCK_PATH = STATE_DIR / "slots.lock"


def pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def main() -> int:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    if not SLOTS_PATH.exists():
        SLOTS_PATH.write_text("[]\n", encoding="ascii")
    with LOCK_PATH.open("a+", encoding="ascii") as lock_file:
        fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
        try:
            data = json.loads(SLOTS_PATH.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            data = []
        live = []
        removed = []
        for item in data:
            pid = item.get("pid")
            if isinstance(pid, int) and pid_alive(pid):
                live.append(item)
            else:
                removed.append(item)
        SLOTS_PATH.write_text(json.dumps(live, indent=2) + "\n", encoding="utf-8")
        fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)
    print(json.dumps({"removed": removed, "remaining": live}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
