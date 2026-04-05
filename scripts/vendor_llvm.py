#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "vendor" / "llvm" / "manifest.json"
SHIM_SOURCE = ROOT / "scripts" / "llvm_config_shim.rs"
LLVM_ROOT = ROOT / "vendor" / "llvm"


def load_manifest() -> dict:
    return json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))


def detect_host_target() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "linux" and machine in {"x86_64", "amd64"}:
        return "linux-x86_64"
    if system == "linux" and machine in {"aarch64", "arm64"}:
        return "linux-aarch64"
    if system == "darwin" and machine in {"aarch64", "arm64"}:
        return "macos-aarch64"
    if system == "darwin" and machine in {"x86_64", "amd64"}:
        return "macos-x86_64"
    if system == "windows" and machine in {"x86_64", "amd64"}:
        return "windows-x86_64"
    raise SystemExit(f"unsupported host platform: system={system!r} machine={machine!r}")


def normalize_layout(root_dir: Path, target_dir: Path) -> None:
    target_dir.mkdir(parents=True, exist_ok=True)
    for name in ("include", "lib", "bin"):
        source = root_dir / name
        dest = target_dir / name
        if dest.exists():
            shutil.rmtree(dest)
        if source.exists():
            shutil.copytree(source, dest)


def fetch_archive(url: str, path: Path) -> None:
    with urllib.request.urlopen(url) as response, path.open("wb") as handle:
        shutil.copyfileobj(response, handle)


def extract_archive(archive_path: Path, destination: Path) -> Path:
    with tarfile.open(archive_path, "r:*") as tar:
        tar.extractall(destination)
    children = [entry for entry in destination.iterdir() if entry.is_dir()]
    if len(children) != 1:
        raise SystemExit(f"expected one extracted root in {destination}, found {len(children)}")
    return children[0]


def cmd_fetch(target: str) -> int:
    manifest = load_manifest()
    entry = manifest["targets"][target]
    archives_dir = LLVM_ROOT / "_archives"
    archives_dir.mkdir(parents=True, exist_ok=True)
    archive_path = archives_dir / entry["archive"]
    extract_dir = LLVM_ROOT / "_extract" / target
    target_dir = LLVM_ROOT / target

    if not archive_path.exists():
        print(f"downloading {entry['url']} -> {archive_path}", file=sys.stderr)
        fetch_archive(entry["url"], archive_path)

    if extract_dir.exists():
        shutil.rmtree(extract_dir)
    extract_dir.mkdir(parents=True)
    extracted_root = extract_archive(archive_path, extract_dir)
    if extracted_root.name != entry["root_dir"]:
        raise SystemExit(
            f"unexpected root dir: expected {entry['root_dir']}, found {extracted_root.name}"
        )
    normalize_layout(extracted_root, target_dir)
    prepare_llvm_config(target_dir, target)
    ensure_host_alias(target)
    print(target_dir)
    return 0


def emit_env(prefix: Path, target: str, format_name: str) -> None:
    llvm_config = prefix / "bin" / ("llvm-config.exe" if target.startswith("windows-") else "llvm-config")
    values = {
        "LLVM_SYS_181_PREFIX": str(prefix),
        "LLVM_CONFIG_PATH": str(llvm_config),
        "DRATON_LLVM_BUNDLE_PREFIX": str(prefix),
    }
    if format_name == "shell":
        for key, value in values.items():
            print(f"export {key}='{value}'")
        return
    if format_name == "github":
        for key, value in values.items():
            print(f"{key}={value}")
        return
    if format_name == "json":
        print(json.dumps(values))
        return
    raise SystemExit(f"unsupported print-env format: {format_name}")


def cmd_print_env(target: str, format_name: str) -> int:
    prefix = LLVM_ROOT / target
    prepare_llvm_config(prefix, target)
    ensure_host_alias(target)
    emit_env(prefix, target, format_name)
    return 0


def llvm_config_name(target: str) -> str:
    return "llvm-config.exe" if target.startswith("windows-") else "llvm-config"


def llvm_config_real_name(target: str) -> str:
    return "llvm-config-real.exe" if target.startswith("windows-") else "llvm-config-real"


def shim_marker_name(target: str) -> str:
    return ".llvm-config-shim.exe.stamp" if target.startswith("windows-") else ".llvm-config-shim.stamp"


def extracted_llvm_config_path(target: str) -> Path | None:
    manifest = load_manifest()
    entry = manifest["targets"].get(target)
    if entry is None:
        return None
    path = LLVM_ROOT / "_extract" / target / entry["root_dir"] / "bin" / llvm_config_name(target)
    return path if path.exists() else None


def build_llvm_config_shim(target: str, shim_path: Path) -> None:
    rustc = resolve_rustc()
    if rustc is None:
        raise SystemExit("rustc is required to prepare the vendored llvm-config shim")
    shim_path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = shim_path.with_suffix(f"{shim_path.suffix}.tmp")
    subprocess.run(
        [rustc, "--edition=2021", "-O", str(SHIM_SOURCE), "-o", str(temp_path)],
        check=True,
        cwd=ROOT,
    )
    temp_path.replace(shim_path)


def resolve_rustc() -> str | None:
    explicit = os.environ.get("RUSTC")
    if explicit:
        candidate = Path(explicit).expanduser()
        if candidate.exists():
            return str(candidate)
    for candidate in (
        Path.home() / ".cargo" / "bin" / "rustc",
        Path("/home/lehungquangminh/.cargo/bin/rustc"),
    ):
        if candidate.exists():
            return str(candidate)
    return shutil.which("rustc")


def prepare_llvm_config(prefix: Path, target: str) -> None:
    if not prefix.exists():
        return
    bin_dir = prefix / "bin"
    if not bin_dir.exists():
        return

    shim_path = bin_dir / llvm_config_name(target)
    real_path = bin_dir / llvm_config_real_name(target)
    marker_path = bin_dir / shim_marker_name(target)

    if not real_path.exists():
        extracted = extracted_llvm_config_path(target)
        if extracted is not None:
            shutil.copy2(extracted, real_path)
        elif shim_path.exists() and not marker_path.exists():
            shutil.move(shim_path, real_path)

    needs_compile = not shim_path.exists() or not marker_path.exists()
    if not needs_compile and SHIM_SOURCE.stat().st_mtime > shim_path.stat().st_mtime:
        needs_compile = True
    if needs_compile:
        build_llvm_config_shim(target, shim_path)
        marker_path.write_text(f"shim-source={SHIM_SOURCE}\n", encoding="utf-8")


def ensure_host_alias(target: str) -> None:
    host_target = detect_host_target()
    if target != host_target:
        return

    target_dir = LLVM_ROOT / target
    if not target_dir.exists():
        return

    alias = LLVM_ROOT / "host"
    desired = Path(target)
    try:
        if alias.is_symlink() or alias.is_file():
            alias.unlink()
        elif alias.is_dir():
            shutil.rmtree(alias)
        alias.symlink_to(desired, target_is_directory=True)
    except OSError:
        # Best-effort alias on hosts that do not support directory symlinks cleanly.
        return


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Manage vendored LLVM bundles for Draton.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    fetch = subparsers.add_parser("fetch", help="download and normalize a vendored LLVM bundle")
    fetch.add_argument("--target", default="host")

    env = subparsers.add_parser("print-env", help="print shell exports for the selected LLVM bundle")
    env.add_argument("--target", default="host")
    env.add_argument("--format", choices=("shell", "github", "json"), default="shell")
    return parser.parse_args()


def resolve_target(value: str) -> str:
    return detect_host_target() if value == "host" else value


def main() -> int:
    args = parse_args()
    target = resolve_target(args.target)
    if args.command == "fetch":
        return cmd_fetch(target)
    if args.command == "print-env":
        return cmd_print_env(target, args.format)
    raise SystemExit(f"unknown command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
