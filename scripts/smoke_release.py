#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

SCRUBBED_TOOLCHAIN_ENV_KEYS = (
    "AR",
    "CC",
    "CPP",
    "CXX",
    "LD",
    "LIB",
    "LIBRARY_PATH",
    "LINK",
    "LLVM_CONFIG_PATH",
    "LLVM_PATH",
    "LLVM_SYS_140_PREFIX",
    "LLVM_SYS_181_PREFIX",
    "NM",
    "RANLIB",
)


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
    print(f"+ {' '.join(cmd)}", flush=True)
    try:
        completed = subprocess.run(cmd, cwd=cwd, env=env, check=False, text=True, capture_output=True)
    except FileNotFoundError as exc:
        raise SystemExit(f"failed to start command: {' '.join(cmd)} ({exc})") from exc
    if completed.returncode != 0:
        sys.stderr.write(completed.stdout)
        sys.stderr.write(completed.stderr)
        raise SystemExit(f"command failed with exit code {completed.returncode}: {' '.join(cmd)}")


def sanitized_runtime_env(root: Path) -> dict[str, str]:
    env = os.environ.copy()
    llvm_path = env.get("LLVM_PATH")
    filtered = []
    for entry in env.get("PATH", "").split(os.pathsep):
        if not entry:
            continue
        entry_lower = entry.lower()
        if llvm_path and os.path.normcase(entry).startswith(os.path.normcase(llvm_path)):
            continue
        if "llvm" in entry_lower:
            continue
        filtered.append(entry)
    path_entries = [str(root)]
    packaged_llvm_bin = root / "llvm" / "bin"
    if packaged_llvm_bin.exists():
        path_entries.append(str(packaged_llvm_bin))
        env["DRATON_LLVM_BUNDLE_PREFIX"] = str(root / "llvm")
    env["PATH"] = os.pathsep.join([*path_entries, *filtered])
    for key in SCRUBBED_TOOLCHAIN_ENV_KEYS:
        env.pop(key, None)
    packaged_llvm_lib = root / "llvm" / "lib"
    if packaged_llvm_lib.exists():
        if sys.platform.startswith("linux"):
            env["LD_LIBRARY_PATH"] = str(packaged_llvm_lib)
        elif sys.platform == "darwin":
            env["DYLD_LIBRARY_PATH"] = str(packaged_llvm_lib)
    else:
        env.pop("LD_LIBRARY_PATH", None)
        env.pop("DYLD_LIBRARY_PATH", None)
    return env


def resolve_binary_path(base: Path) -> Path:
    candidates = [base]
    if base.suffix.lower() != ".exe":
        candidates.append(base.with_suffix(".exe"))
    for candidate in candidates:
        if candidate.exists():
            return candidate
    raise SystemExit(f"expected built binary at one of: {', '.join(str(path) for path in candidates)}")


def assert_no_llvm_runtime(binary: Path) -> None:
    if sys.platform.startswith("linux") and shutil.which("ldd"):
        completed = subprocess.run(
            ["ldd", str(binary)],
            text=True,
            capture_output=True,
            check=False,
        )
        deps = completed.stdout + completed.stderr
        if "libLLVM" in deps or "libclang-cpp" in deps:
            raise SystemExit("packaged release still depends on an external LLVM shared library")
    elif sys.platform == "darwin" and shutil.which("otool"):
        completed = subprocess.run(
            ["otool", "-L", str(binary)],
            text=True,
            capture_output=True,
            check=False,
        )
        deps = completed.stdout + completed.stderr
        if "LLVM" in deps or "clang-cpp" in deps:
            raise SystemExit("packaged release still depends on an external LLVM shared library")


def smoke_lsp(binary: Path, cwd: Path, env: dict[str, str]) -> None:
    payload = json.dumps(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"capabilities": {}, "rootUri": None},
        }
    )
    message = f"Content-Length: {len(payload)}\r\n\r\n{payload}".encode("utf-8")
    cmd = [str(binary), "lsp"]
    print(f"+ {' '.join(cmd)} <initialize>", flush=True)
    completed = subprocess.run(
        cmd,
        cwd=cwd,
        env={**os.environ, **env},
        input=message,
        capture_output=True,
        check=False,
        timeout=30,
    )
    if completed.returncode != 0:
        sys.stderr.write(completed.stdout.decode(errors="replace"))
        sys.stderr.write(completed.stderr.decode(errors="replace"))
        raise SystemExit(f"command failed with exit code {completed.returncode}: {' '.join(cmd)}")
    if b"completionProvider" not in completed.stdout:
        raise SystemExit("lsp initialize smoke test did not return capabilities")


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
        assert_no_llvm_runtime(binary)

        env = sanitized_runtime_env(root)

        run([str(binary), "--version"], cwd=root, extra_env=env)
        run([str(binary), "fmt", "--check", "examples/early-preview/hello-app/src"], cwd=root, extra_env=env)
        run([str(binary), "lint", "examples/early-preview/hello-app/src"], cwd=root, extra_env=env)
        example_root = root / "examples" / "early-preview" / "hello-app"
        run([str(binary), "task"], cwd=example_root, extra_env=env)
        run([str(binary), "task", "build"], cwd=example_root, extra_env=env)
        built_example = resolve_binary_path(example_root / "build" / "hello-preview")
        run([str(built_example)], cwd=example_root, extra_env=env)
        run([str(binary), "build"], cwd=example_root, extra_env=env)
        built_project = resolve_binary_path(example_root / "build" / "hello-preview")
        run([str(built_project)], cwd=example_root, extra_env=env)
        run([str(binary), "run"], cwd=example_root, extra_env=env)
        output_binary = root / "hello-tooling"
        run(
            [str(binary), "build", "examples/hello.dt", "-o", str(output_binary)],
            cwd=root,
            extra_env=env,
        )
        output_binary = resolve_binary_path(output_binary)
        run([str(output_binary)], cwd=root, extra_env=env)
        run([str(binary), "run", "examples/hello.dt"], cwd=root, extra_env=env)
        smoke_lsp(binary, root, env)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
