from __future__ import annotations

import importlib.util
import unittest
from argparse import Namespace
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def load_module(name: str, relative_path: str):
    path = REPO_ROOT / relative_path
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load module from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


remote_phase1 = load_module("remote_phase1", "scripts/run_remote_phase1.py")


class RemotePhase1Tests(unittest.TestCase):
    def test_workflow_inputs_encode_booleans_as_github_dispatch_strings(self) -> None:
        args = Namespace(parse_slice=True, release_smoke=False)
        self.assertEqual(
            remote_phase1.workflow_inputs(args),
            {
                "parse_slice": "true",
                "release_smoke": "false",
            },
        )

    def test_build_workflow_run_command_uses_filename_ref_and_inputs(self) -> None:
        command = remote_phase1.build_workflow_run_command(
            "codex/test-branch",
            {
                "parse_slice": "true",
                "release_smoke": "false",
            },
        )
        self.assertEqual(
            command,
            [
                "gh",
                "workflow",
                "run",
                "ci.yml",
                "--ref",
                "codex/test-branch",
                "-f",
                "parse_slice=true",
                "-f",
                "release_smoke=false",
            ],
        )


if __name__ == "__main__":
    unittest.main()
