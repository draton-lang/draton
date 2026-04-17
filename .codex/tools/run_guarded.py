#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shlex
import signal
import subprocess
import sys
import time
from pathlib import Path

if os.name == "nt":
    import msvcrt
    import ctypes

    resource = None
else:
    import fcntl
    import resource


SCRIPT_DIR = Path(__file__).resolve().parent
STATE_DIR = SCRIPT_DIR / ".state"
SLOTS_PATH = STATE_DIR / "slots.json"
LOCK_PATH = STATE_DIR / "slots.lock"

DEFAULT_TIMEOUT_SEC = 900
DEFAULT_WAIT_SEC = 120
DEFAULT_CONCURRENCY = 2
DEFAULT_MEMORY_MB = 2048
DEFAULT_CPU_SECONDS = 600
DEFAULT_FILE_SIZE_MB = 64

if os.name == "nt":
    LOCK_BYTES = 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run a command with timeout, memory, CPU, and concurrency guards."
    )
    parser.add_argument("--cwd", default=".", help="Working directory for the command.")
    parser.add_argument("--timeout-sec", type=int, default=DEFAULT_TIMEOUT_SEC)
    parser.add_argument("--wait-sec", type=int, default=DEFAULT_WAIT_SEC)
    parser.add_argument("--concurrency", type=int, default=DEFAULT_CONCURRENCY)
    parser.add_argument("--memory-mb", type=int, default=DEFAULT_MEMORY_MB)
    parser.add_argument("--cpu-seconds", type=int, default=DEFAULT_CPU_SECONDS)
    parser.add_argument("--file-size-mb", type=int, default=DEFAULT_FILE_SIZE_MB)
    parser.add_argument("--nice", type=int, default=10)
    parser.add_argument(
        "--env",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="Extra environment variables to add or override.",
    )
    parser.add_argument(
        "--json-only",
        action="store_true",
        help="Emit only the JSON result block.",
    )
    parser.add_argument(
        "command",
        nargs=argparse.REMAINDER,
        help="Command to execute. Pass after --, for example: -- cargo test -p draton-parser --test items",
    )
    args = parser.parse_args()
    while args.command and args.command[0] == "--":
        args.command = args.command[1:]
    if not args.command:
        parser.error("a command is required after --")
    return args


def ensure_state_dir() -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    if not SLOTS_PATH.exists():
        SLOTS_PATH.write_text("[]\n", encoding="ascii")


def pid_alive(pid: int) -> bool:
    if os.name == "nt":
        # PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
        handle = ctypes.windll.kernel32.OpenProcess(0x1000, False, pid)
        if handle == 0:
            return False
        ctypes.windll.kernel32.CloseHandle(handle)
        return True
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def load_slots() -> list[dict[str, object]]:
    try:
        data = json.loads(SLOTS_PATH.read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError):
        return []
    live: list[dict[str, object]] = []
    for item in data:
        pid = item.get("pid")
        if isinstance(pid, int) and pid_alive(pid):
            live.append(item)
    return live


def save_slots(slots: list[dict[str, object]]) -> None:
    SLOTS_PATH.write_text(json.dumps(slots, indent=2) + "\n", encoding="utf-8")


class FileLock:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.handle = None

    def __enter__(self):
        self.handle = self.path.open("a+", encoding="ascii")
        if os.name == "nt":
            self.handle.seek(0)
            msvcrt.locking(self.handle.fileno(), msvcrt.LK_LOCK, LOCK_BYTES)
        else:
            fcntl.flock(self.handle.fileno(), fcntl.LOCK_EX)
        return self.handle

    def __exit__(self, exc_type, exc, tb) -> None:
        if self.handle is None:
            return
        try:
            if os.name == "nt":
                self.handle.seek(0)
                msvcrt.locking(self.handle.fileno(), msvcrt.LK_UNLCK, LOCK_BYTES)
            else:
                fcntl.flock(self.handle.fileno(), fcntl.LOCK_UN)
        finally:
            self.handle.close()
            self.handle = None


class SlotClaim:
    def __init__(self, token: dict[str, object]) -> None:
        self.token = token
        self.active = True

    def release(self) -> None:
        if not self.active:
            return
        ensure_state_dir()
        with FileLock(LOCK_PATH):
            slots = load_slots()
            slots = [item for item in slots if item != self.token]
            save_slots(slots)
        self.active = False


def acquire_slot(concurrency: int, wait_sec: int, cwd: str, command: list[str]) -> SlotClaim:
    ensure_state_dir()
    deadline = time.time() + wait_sec
    token = {
        "pid": os.getpid(),
        "started_at": int(time.time()),
        "cwd": cwd,
        "command": " ".join(shlex.quote(part) for part in command),
    }
    while True:
        with FileLock(LOCK_PATH):
            slots = load_slots()
            if len(slots) < concurrency:
                slots.append(token)
                save_slots(slots)
                return SlotClaim(token)
        if time.time() >= deadline:
            raise TimeoutError(
                f"timed out waiting for a free guarded slot after {wait_sec}s"
            )
        time.sleep(1.0)


def build_env(overrides: list[str]) -> dict[str, str]:
    env = os.environ.copy()
    for item in overrides:
        if "=" not in item:
            raise ValueError(f"invalid env override: {item!r}")
        key, value = item.split("=", 1)
        env[key] = value
    return env


def make_preexec(memory_mb: int, cpu_seconds: int, file_size_mb: int, nice_value: int):
    memory_bytes = memory_mb * 1024 * 1024
    file_size_bytes = file_size_mb * 1024 * 1024

    def _preexec() -> None:
        os.setsid()
        try:
            os.nice(nice_value)
        except OSError:
            pass
        if resource is None:
            return
        resource.setrlimit(resource.RLIMIT_AS, (memory_bytes, memory_bytes))
        resource.setrlimit(resource.RLIMIT_CPU, (cpu_seconds, cpu_seconds))
        resource.setrlimit(resource.RLIMIT_FSIZE, (file_size_bytes, file_size_bytes))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))

    return _preexec


def kill_process_group(proc: subprocess.Popen[str]) -> None:
    if os.name == "nt":
        try:
            proc.terminate()
        except ProcessLookupError:
            return
        except OSError:
            return
        time.sleep(0.2)
        if proc.poll() is None:
            try:
                proc.kill()
            except ProcessLookupError:
                pass
            except OSError:
                pass
        return
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    time.sleep(0.2)
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def main() -> int:
    args = parse_args()
    cwd = str(Path(args.cwd).resolve())
    claim = acquire_slot(args.concurrency, args.wait_sec, cwd, args.command)
    started = time.time()
    try:
        env = build_env(args.env)
        popen_kwargs: dict[str, object] = {
            "cwd": cwd,
            "env": env,
            "stdin": subprocess.DEVNULL,
            "stdout": subprocess.PIPE,
            "stderr": subprocess.PIPE,
            "text": True,
        }
        if os.name == "nt":
            popen_kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
        else:
            popen_kwargs["preexec_fn"] = make_preexec(
                args.memory_mb, args.cpu_seconds, args.file_size_mb, args.nice
            )
        proc = subprocess.Popen(
            args.command,
            **popen_kwargs,
        )
        timed_out = False
        try:
            stdout, stderr = proc.communicate(timeout=args.timeout_sec)
        except subprocess.TimeoutExpired:
            timed_out = True
            kill_process_group(proc)
            stdout, stderr = proc.communicate()
        result = {
            "command": args.command,
            "cwd": cwd,
            "returncode": proc.returncode,
            "timed_out": timed_out,
            "timeout_sec": args.timeout_sec,
            "memory_mb": args.memory_mb,
            "cpu_seconds": args.cpu_seconds,
            "file_size_mb": args.file_size_mb,
            "duration_sec": round(time.time() - started, 3),
            "stdout": stdout,
            "stderr": stderr,
        }
        output = json.dumps(result, indent=2)
        if args.json_only:
            print(output)
        else:
            print(output)
        if timed_out:
            return 124
        return proc.returncode
    finally:
        claim.release()


if __name__ == "__main__":
    raise SystemExit(main())
