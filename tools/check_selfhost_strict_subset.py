#!/usr/bin/env python3
"""Enforce canonical syntax on the migrated self-host subset.

This check is intentionally narrower than a full-tree self-host strict build.
It scans the self-host mirror under `src/` and allows deprecated inline type
syntax only in the explicitly tracked blocker files.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = ROOT / "src"

EXCLUDED_FILES = {
    Path("src/ast/dump.dt"),
    Path("src/typeck/dump.dt"),
}

PATTERNS = {
    "typed_let": re.compile(
        r"\blet\s+(?:mut\s+)?[A-Za-z_][A-Za-z0-9_]*\s*:\s*[^=\n]+=",
        re.MULTILINE,
    ),
    "typed_param": re.compile(
        r"\bfn\s+[A-Za-z_][A-Za-z0-9_]*\s*(?:\[[^\]]*\])?\s*\([^)]*:[^)]*\)",
        re.MULTILINE,
    ),
    "typed_return": re.compile(
        r"\bfn\s+[A-Za-z_][A-Za-z0-9_]*\s*(?:\[[^\]]*\])?\s*\([^)]*\)\s*->",
        re.MULTILINE,
    ),
}


@dataclass(frozen=True)
class Finding:
    kind: str
    line: int
    col: int
    snippet: str


def line_col(text: str, index: int) -> tuple[int, int]:
    line = text.count("\n", 0, index) + 1
    last_newline = text.rfind("\n", 0, index)
    col = index + 1 if last_newline < 0 else index - last_newline
    return line, col


def collect_findings(path: Path) -> list[Finding]:
    text = path.read_text(encoding="utf-8")
    findings: list[Finding] = []
    for kind, pattern in PATTERNS.items():
        for match in pattern.finditer(text):
            line, col = line_col(text, match.start())
            snippet = match.group(0).strip().splitlines()[0]
            findings.append(Finding(kind=kind, line=line, col=col, snippet=snippet))
    findings.sort(key=lambda finding: (finding.line, finding.col, finding.kind))
    return findings


def main() -> int:
    missing_exclusions = [
        rel for rel in sorted(EXCLUDED_FILES) if not (ROOT / rel).exists()
    ]
    if missing_exclusions:
        print("configured exclusion files are missing:")
        for rel in missing_exclusions:
            print(f"  - {rel}")
        return 1

    violations: dict[Path, list[Finding]] = {}
    excluded_hits: dict[Path, list[Finding]] = {}
    stale_exclusions: list[Path] = []

    src_files = sorted(SRC_ROOT.rglob("*.dt"))
    for path in src_files:
        rel = path.relative_to(ROOT)
        findings = collect_findings(path)
        if rel in EXCLUDED_FILES:
            if findings:
                excluded_hits[rel] = findings
            else:
                stale_exclusions.append(rel)
            continue
        if findings:
            violations[rel] = findings

    covered = len(src_files) - len(EXCLUDED_FILES)
    print(
        f"checked {covered} migrated self-host files; "
        f"{len(EXCLUDED_FILES)} files remain explicitly excluded"
    )

    if excluded_hits:
        print("tracked exclusions:")
        for rel, findings in sorted(excluded_hits.items()):
            kinds = ", ".join(sorted({finding.kind for finding in findings}))
            print(f"  - {rel} ({kinds})")

    if violations:
        print("\nnon-excluded files still contain deprecated inline type syntax:")
        for rel, findings in sorted(violations.items()):
            print(f"  - {rel}")
            for finding in findings:
                print(
                    f"      {finding.kind} at {finding.line}:{finding.col}: "
                    f"{finding.snippet}"
                )
        return 1

    if stale_exclusions:
        print("\nstale exclusions detected; remove them from the subset list and docs:")
        for rel in stale_exclusions:
            print(f"  - {rel}")
        return 1

    print("\nself-host strict-canonical subset is clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
