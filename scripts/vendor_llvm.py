#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import platform
import shutil
import sys
import tarfile
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "vendor" / "llvm" / "manifest.json"


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
    llvm_root = ROOT / "vendor" / "llvm"
    archives_dir = llvm_root / "_archives"
    archives_dir.mkdir(parents=True, exist_ok=True)
    archive_path = archives_dir / entry["archive"]
    extract_dir = llvm_root / "_extract" / target
    target_dir = llvm_root / target

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
    print(target_dir)
    return 0


def cmd_print_env(target: str) -> int:
    prefix = ROOT / "vendor" / "llvm" / target
    print(f"export LLVM_SYS_181_PREFIX='{prefix}'")
    print(f"export LLVM_CONFIG_PATH='{prefix / 'bin' / 'llvm-config'}'")
    print(f"export DRATON_LLVM_BUNDLE_PREFIX='{prefix}'")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Manage vendored LLVM bundles for Draton.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    fetch = subparsers.add_parser("fetch", help="download and normalize a vendored LLVM bundle")
    fetch.add_argument("--target", default="host")

    env = subparsers.add_parser("print-env", help="print shell exports for the selected LLVM bundle")
    env.add_argument("--target", default="host")
    return parser.parse_args()


def resolve_target(value: str) -> str:
    return detect_host_target() if value == "host" else value


def main() -> int:
    args = parse_args()
    target = resolve_target(args.target)
    if args.command == "fetch":
        return cmd_fetch(target)
    if args.command == "print-env":
        return cmd_print_env(target)
    raise SystemExit(f"unknown command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
