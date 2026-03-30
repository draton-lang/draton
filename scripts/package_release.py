#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import stat
import subprocess
import sys
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


def resolve_llvm_bundle_root() -> Path | None:
    for key in ("DRATON_LLVM_BUNDLE_PREFIX", "LLVM_PATH"):
        value = os.environ.get(key)
        if value:
            path = Path(value).resolve()
            if path.exists():
                return path
    return None


def copy_release_files(staging_root: Path, binary: Path, runtime_lib: Path) -> None:
    repo_root = Path(__file__).resolve().parent.parent
    staging_root.mkdir(parents=True, exist_ok=True)
    bin_name = "drat.exe" if binary.suffix.lower() == ".exe" else "drat"
    staged_binary = staging_root / bin_name
    shutil.copy2(binary, staged_binary)
    if staged_binary.suffix.lower() != ".exe":
        ensure_executable(staged_binary)

    shutil.copy2(runtime_lib, staging_root / runtime_lib.name)
    if binary.suffix.lower() == ".exe":
        extra_runtime = binary.parent / "libdraton_runtime.a"
        if extra_runtime.exists() and extra_runtime.resolve() != runtime_lib.resolve():
            shutil.copy2(extra_runtime, staging_root / extra_runtime.name)
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
    bundle_llvm_toolchain(staging_root)
    bundle_windows_gnu_toolchain(staging_root)
    bundle_windows_runtime_libs(staging_root, staged_binary)
    bundle_macos_runtime_libs(staging_root, staged_binary)


def bundle_llvm_toolchain(staging_root: Path) -> None:
    llvm_root = resolve_llvm_bundle_root()
    if llvm_root is None:
        return

    target_root = staging_root / "llvm"
    for relative in ("bin", "lib"):
        source = llvm_root / relative
        if not source.exists():
            continue
        shutil.copytree(source, target_root / relative, dirs_exist_ok=True)


def bundle_windows_gnu_toolchain(staging_root: Path) -> None:
    if sys.platform != "win32":
        return

    mingw_root = os.environ.get("DRATON_WINDOWS_GNU_ROOT")
    if not mingw_root:
        candidate = Path("C:/mingw64")
        if candidate.exists():
            mingw_root = str(candidate)
    if not mingw_root:
        return

    source_root = Path(mingw_root)
    if not source_root.exists():
        return

    target_root = staging_root / "windows-gnu"
    for relative in ["bin", "lib", "libexec", "x86_64-w64-mingw32"]:
        source = source_root / relative
        if not source.exists():
            continue
        shutil.copytree(source, target_root / relative, dirs_exist_ok=True)


def bundle_windows_runtime_libs(staging_root: Path, staged_binary: Path) -> None:
    if staged_binary.suffix.lower() != ".exe":
        return

    llvm_root = resolve_llvm_bundle_root()
    if llvm_root is None:
        return

    llvm_bin_dir = llvm_root / "bin"
    if not llvm_bin_dir.exists():
        return

    for dll in llvm_bin_dir.glob("*.dll"):
        shutil.copy2(dll, staging_root / dll.name)


def bundle_macos_runtime_libs(staging_root: Path, staged_binary: Path) -> None:
    if sys.platform != "darwin" or not shutil.which("otool") or not shutil.which("install_name_tool"):
        return

    llvm_root = resolve_llvm_bundle_root()
    if llvm_root is None:
        return

    llvm_lib_dir = llvm_root / "lib"
    if not llvm_lib_dir.exists():
        return

    def inspect_dependencies(path: Path) -> dict[str, str]:
        inspect = subprocess.run(
            ["otool", "-L", str(path)],
            text=True,
            capture_output=True,
            check=False,
        )
        if inspect.returncode != 0:
            return {}
        needed: dict[str, str] = {}
        for line in inspect.stdout.splitlines():
            stripped = line.strip()
            if not stripped:
                continue
            dep = stripped.split(" ", 1)[0]
            name = Path(dep).name
            if name in {"libc++.1.dylib", "libc++abi.1.dylib", "libunwind.1.dylib"}:
                needed[name] = dep
        return needed

    needed_paths = inspect_dependencies(staged_binary)
    if not needed_paths:
        return

    copied_paths: dict[str, Path] = {}
    pending = list(needed_paths)
    while pending:
        name = pending.pop()
        if name in copied_paths:
            continue
        source = llvm_lib_dir / name
        if not source.exists():
            raise SystemExit(f"missing macOS runtime dependency for packaged drat: {source}")
        target = staging_root / name
        shutil.copy2(source, target)
        copied_paths[name] = target
        for nested_name, nested_dep in inspect_dependencies(target).items():
            if nested_name not in needed_paths:
                needed_paths[nested_name] = nested_dep
                pending.append(nested_name)

    subprocess.run(
        ["install_name_tool", "-add_rpath", "@executable_path", str(staged_binary)],
        check=False,
        capture_output=True,
        text=True,
    )

    for name, original_path in needed_paths.items():
        subprocess.run(
            ["install_name_tool", "-change", original_path, f"@executable_path/{name}", str(staged_binary)],
            check=True,
            capture_output=True,
            text=True,
        )

    for name, dylib_path in copied_paths.items():
        subprocess.run(
            ["install_name_tool", "-id", f"@loader_path/{name}", str(dylib_path)],
            check=True,
            capture_output=True,
            text=True,
        )
        for dep_name, dep_path in inspect_dependencies(dylib_path).items():
            subprocess.run(
                ["install_name_tool", "-change", dep_path, f"@loader_path/{dep_name}", str(dylib_path)],
                check=True,
                capture_output=True,
                text=True,
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
    runtime_path = staging_dir / runtime_filename(binary)
    if not runtime_path.exists():
        raise SystemExit(f"package_release: runtime staticlib missing from staging: {runtime_path}")

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
