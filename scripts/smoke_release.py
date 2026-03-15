#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Smoke test a packaged Draton release archive.")
    parser.add_argument("--archive", required=True, help="Path to archive to test")
    return parser.parse_args()


def extract_archive(archive: Path, destination: Path) -> Path:
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as tar:
            tar.extractall(destination)
    elif archive.suffix.lower() == ".zip":
        with zipfile.ZipFile(archive) as zf:
            zf.extractall(destination)
    else:
        raise SystemExit(f"unsupported archive type: {archive}")

    children = [path for path in destination.iterdir() if path.is_dir()]
    if not children:
        raise SystemExit("archive did not contain a root directory")
    return children[0]


def run(cmd: list[str], cwd: Path, extra_env: dict[str, str] | None = None) -> None:
    env = os.environ.copy()
    if extra_env:
        env.update(extra_env)
    completed = subprocess.run(cmd, cwd=cwd, env=env, check=False, text=True, capture_output=True)
    if completed.returncode != 0:
        sys.stderr.write(completed.stdout)
        sys.stderr.write(completed.stderr)
        raise SystemExit(completed.returncode)


def main() -> int:
    args = parse_args()
    archive = Path(args.archive).resolve()
    with tempfile.TemporaryDirectory() as tmpdir:
        root = extract_archive(archive, Path(tmpdir))
        binary = root / ("drat.exe" if archive.suffix.lower() == ".zip" else "drat")
        if not binary.exists():
            raise SystemExit(f"missing packaged binary: {binary}")
        if os.name != "nt":
            binary.chmod(binary.stat().st_mode | 0o111)

        env = {}
        if os.name == "nt":
            env["PATH"] = str(root) + os.pathsep + os.environ.get("PATH", "")

        run([str(binary), "--version"], cwd=root, extra_env=env)
        run([str(binary), "run", "examples/hello.dt"], cwd=root, extra_env=env)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
