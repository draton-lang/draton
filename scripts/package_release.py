#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import stat
import tarfile
import zipfile
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Package a Draton release artifact.")
    parser.add_argument("--binary", required=True, help="Path to the built drat binary")
    parser.add_argument(
        "--runtime-lib",
        required=False,
        help="Path to the packaged runtime static library (defaults to sibling of --binary)",
    )
    parser.add_argument("--artifact", required=True, help="Final archive filename")
    parser.add_argument("--out-dir", required=True, help="Output directory for the archive")
    parser.add_argument(
        "--archive-root",
        required=False,
        help="Root directory inside the archive (defaults to artifact stem)",
    )
    return parser.parse_args()


def ensure_executable(path: Path) -> None:
    mode = path.stat().st_mode
    path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def runtime_filename(binary: Path) -> str:
    return "draton_runtime.lib" if binary.suffix.lower() == ".exe" else "libdraton_runtime.a"


def resolve_runtime_lib(binary: Path, runtime_lib: str | None) -> Path:
    if runtime_lib:
        path = Path(runtime_lib).resolve()
    else:
        path = (binary.parent / runtime_filename(binary)).resolve()
    if not path.exists():
        raise SystemExit(f"missing runtime static library: {path}")
    return path


def copy_release_files(staging_root: Path, binary: Path, runtime_lib: Path) -> None:
    repo_root = Path(__file__).resolve().parent.parent
    staging_root.mkdir(parents=True, exist_ok=True)
    bin_name = "drat.exe" if binary.suffix.lower() == ".exe" else "drat"
    staged_binary = staging_root / bin_name
    shutil.copy2(binary, staged_binary)
    if staged_binary.suffix.lower() != ".exe":
        ensure_executable(staged_binary)

    shutil.copy2(runtime_lib, staging_root / runtime_lib.name)
    shutil.copy2(repo_root / "LICENSE", staging_root / "LICENSE")
    shutil.copy2(repo_root / "QUICKSTART.md", staging_root / "QUICKSTART.md")
    shutil.copy2(repo_root / "docs" / "install.md", staging_root / "INSTALL.md")
    shutil.copy2(repo_root / "docs" / "early-preview.md", staging_root / "EARLY-PREVIEW.md")
    shutil.copy2(repo_root / "install.sh", staging_root / "install.sh")
    shutil.copy2(repo_root / "install.ps1", staging_root / "install.ps1")
    if staged_binary.suffix.lower() != ".exe":
        ensure_executable(staging_root / "install.sh")

    examples_dir = staging_root / "examples"
    examples_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(repo_root / "examples" / "hello.dt", examples_dir / "hello.dt")
    if (repo_root / "examples" / "early-preview").exists():
        shutil.copytree(
            repo_root / "examples" / "early-preview",
            examples_dir / "early-preview",
            dirs_exist_ok=True,
        )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def create_tar_gz(archive_path: Path, archive_root: Path) -> None:
    with tarfile.open(archive_path, "w:gz", compresslevel=6) as tar:
        tar.add(archive_root, arcname=archive_root.name)


def create_zip(archive_path: Path, archive_root: Path) -> None:
    with zipfile.ZipFile(
        archive_path,
        "w",
        compression=zipfile.ZIP_DEFLATED,
        compresslevel=6,
    ) as zf:
        for path in archive_root.rglob("*"):
            if path.is_dir():
                continue
            zf.write(path, arcname=path.relative_to(archive_root.parent))


def main() -> int:
    args = parse_args()
    binary = Path(args.binary).resolve()
    runtime_lib = resolve_runtime_lib(binary, args.runtime_lib)
    out_dir = Path(args.out_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    archive_name = args.artifact
    archive_path = out_dir / archive_name

    archive_root_name = args.archive_root
    if not archive_root_name:
        archive_root_name = archive_name.removesuffix(".tar.gz").removesuffix(".zip")
    staging_dir = out_dir / archive_root_name
    if staging_dir.exists():
        shutil.rmtree(staging_dir)
    copy_release_files(staging_dir, binary, runtime_lib)

    if archive_name.endswith(".tar.gz"):
        create_tar_gz(archive_path, staging_dir)
    elif archive_name.endswith(".zip"):
        create_zip(archive_path, staging_dir)
    else:
        raise SystemExit(f"unsupported archive type: {archive_name}")

    checksum_path = archive_path.with_suffix(archive_path.suffix + ".sha256")
    checksum_path.write_text(f"{sha256(archive_path)}  {archive_path.name}\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
