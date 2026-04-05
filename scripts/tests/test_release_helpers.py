from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]


def load_module(name: str, relative_path: str):
    path = REPO_ROOT / relative_path
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load module from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


package_release = load_module("package_release", "scripts/package_release.py")
smoke_release = load_module("smoke_release", "scripts/smoke_release.py")
vendor_llvm = load_module("vendor_llvm", "scripts/vendor_llvm.py")


class PackageReleaseTests(unittest.TestCase):
    def test_resolve_llvm_bundle_root_prefers_bundle_prefix(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            temp_root = Path(tmpdir)
            preferred = temp_root / "bundle"
            fallback = temp_root / "llvm-path"
            preferred.mkdir()
            fallback.mkdir()
            with mock.patch.dict(
                os.environ,
                {
                    "DRATON_LLVM_BUNDLE_PREFIX": str(preferred),
                    "LLVM_PATH": str(fallback),
                },
                clear=False,
            ):
                resolved = package_release.resolve_llvm_bundle_root()
            self.assertEqual(resolved, preferred.resolve())

    def test_bundle_llvm_toolchain_copies_bin_and_lib(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            temp_root = Path(tmpdir)
            llvm_root = temp_root / "llvm-src"
            staging_root = temp_root / "staging"
            (llvm_root / "bin").mkdir(parents=True)
            (llvm_root / "lib").mkdir(parents=True)
            (llvm_root / "bin" / "clang").write_text("clang", encoding="utf-8")
            (llvm_root / "lib" / "libLLVM.so").write_text("llvm", encoding="utf-8")
            with mock.patch.dict(
                os.environ,
                {"DRATON_LLVM_BUNDLE_PREFIX": str(llvm_root)},
                clear=False,
            ):
                package_release.bundle_llvm_toolchain(staging_root)
            self.assertTrue((staging_root / "llvm" / "bin" / "clang").exists())
            self.assertTrue((staging_root / "llvm" / "lib" / "libLLVM.so").exists())


class VendorLlvmTests(unittest.TestCase):
    def test_emit_env_supports_github_format(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            prefix = Path(tmpdir) / "llvm-root"
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                vendor_llvm.emit_env(prefix, "linux-x86_64", "github")
            self.assertEqual(
                buf.getvalue().splitlines(),
                [
                    f"LLVM_SYS_181_PREFIX={prefix}",
                    f"LLVM_CONFIG_PATH={prefix / 'bin' / 'llvm-config'}",
                    f"DRATON_LLVM_BUNDLE_PREFIX={prefix}",
                ],
            )

    def test_prepare_llvm_config_installs_real_binary_and_compiles_shim(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            temp_root = Path(tmpdir)
            prefix = temp_root / "vendor" / "llvm" / "linux-x86_64"
            (prefix / "bin").mkdir(parents=True)
            source = temp_root / "extract" / "bin" / "llvm-config"
            source.parent.mkdir(parents=True)
            source.write_text("real llvm-config", encoding="utf-8")

            captured: list[list[str]] = []

            def fake_run(cmd: list[str], check: bool, cwd: Path) -> None:
                captured.append(cmd)
                self.assertTrue(check)
                self.assertEqual(cwd, REPO_ROOT)
                Path(cmd[-1]).write_text("compiled shim", encoding="utf-8")

            with mock.patch.object(vendor_llvm, "resolve_rustc", return_value="/usr/bin/rustc"):
                with mock.patch.object(vendor_llvm, "extracted_llvm_config_path", return_value=source):
                    with mock.patch.object(vendor_llvm.subprocess, "run", side_effect=fake_run):
                        vendor_llvm.prepare_llvm_config(prefix, "linux-x86_64")

            self.assertEqual((prefix / "bin" / "llvm-config-real").read_text(encoding="utf-8"), "real llvm-config")
            self.assertEqual((prefix / "bin" / "llvm-config").read_text(encoding="utf-8"), "compiled shim")
            self.assertTrue((prefix / "bin" / ".llvm-config-shim.stamp").exists())
            self.assertEqual(len(captured), 1)
            self.assertEqual(captured[0][0], "/usr/bin/rustc")
            self.assertEqual(captured[0][-1], str(prefix / "bin" / "llvm-config.tmp"))

    def test_ensure_host_alias_points_to_normalized_host_target(self) -> None:
        if os.name == "nt":
            self.skipTest("host symlink checks are POSIX-only in this repo")

        with tempfile.TemporaryDirectory() as tmpdir:
            temp_root = Path(tmpdir)
            llvm_root = temp_root / "vendor" / "llvm"
            target_dir = llvm_root / "linux-x86_64"
            target_dir.mkdir(parents=True)

            with mock.patch.object(vendor_llvm, "LLVM_ROOT", llvm_root):
                with mock.patch.object(vendor_llvm, "detect_host_target", return_value="linux-x86_64"):
                    vendor_llvm.ensure_host_alias("linux-x86_64")

            alias = llvm_root / "host"
            self.assertTrue(alias.is_symlink())
            self.assertEqual(alias.resolve(), target_dir.resolve())


class SmokeReleaseTests(unittest.TestCase):
    def test_sanitized_runtime_env_prefers_packaged_llvm_and_scrubs_toolchain_vars(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            temp_root = Path(tmpdir)
            root = temp_root / "archive"
            packaged_llvm = root / "llvm"
            (packaged_llvm / "bin").mkdir(parents=True)
            (packaged_llvm / "lib").mkdir(parents=True)

            with mock.patch.dict(
                os.environ,
                {
                    "PATH": os.pathsep.join(
                        [
                            "/usr/bin",
                            "/opt/llvm/bin",
                            "/toolchains/clang/bin",
                        ]
                    ),
                    "LLVM_PATH": "/opt/llvm",
                    "LLVM_SYS_181_PREFIX": "/opt/llvm-18",
                    "LLVM_CONFIG_PATH": "/opt/llvm/bin/llvm-config",
                    "CC": "clang",
                    "CXX": "clang++",
                    "LD": "ld.lld",
                    "LIBRARY_PATH": "/opt/llvm/lib",
                },
                clear=False,
            ):
                env = smoke_release.sanitized_runtime_env(root)

            path_entries = env["PATH"].split(os.pathsep)
            self.assertEqual(path_entries[0], str(root))
            self.assertEqual(path_entries[1], str(packaged_llvm / "bin"))
            self.assertNotIn("/opt/llvm/bin", path_entries)
            self.assertEqual(env["DRATON_LLVM_BUNDLE_PREFIX"], str(packaged_llvm))
            self.assertEqual(env["DRATON_REQUIRE_BUNDLED_TOOLCHAIN"], "1")
            for key in ("CC", "CXX", "LD", "LLVM_PATH", "LLVM_CONFIG_PATH", "LLVM_SYS_181_PREFIX"):
                self.assertNotIn(key, env)
            if sys.platform.startswith("linux"):
                self.assertEqual(env["LD_LIBRARY_PATH"], str(packaged_llvm / "lib"))
            elif sys.platform == "darwin":
                self.assertEqual(env["DYLD_LIBRARY_PATH"], str(packaged_llvm / "lib"))


if __name__ == "__main__":
    unittest.main()
