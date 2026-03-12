#!/usr/bin/env python3
from __future__ import annotations

import re
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE_FILE = ROOT / "src/source/file.dt"
RESULT_FILE = ROOT / "results/selfhost_phase1_corpus.txt"
TEMP_CASE = ROOT / ".tmp_selfhost_phase1_case.dt"
DRAT = ROOT / "target/debug/drat"
SELFHOST = ROOT / "build/debug/draton-selfhost-phase1"

CASES: list[tuple[str, str]] = [
    ("01_constant_fn", "fn main() { 7 }"),
    (
        "02_keywords",
        "let mut fn return if else for while in match class layer extends implements interface "
        "enum error pub import as spawn chan const lambda true false None",
    ),
    ("03_punctuation", "() {} [] , ; : . .."),
    ("04_ops", "+ += ++ - -= -- -> * *= / /= = == =>"),
    ("05_comments", "///docs\nname // skip\nnext"),
    ("06_signature", "pub fn add(a: Int, b: Int) -> Int { return a }"),
    ("07_collection_like", "[alpha, beta]; foo.bar .. baz"),
    ("08_match_like", "match value { true => one false => two }"),
]


def run(cmd: list[str], *, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


def escape_drat_string(value: str) -> str:
    escaped = (
        value.replace("\\", "\\\\")
        .replace("\"", "\\\"")
    )
    return escaped


def build_content_statements(content: str) -> str:
    lines = ['    let mut content = ""']
    chunk = []

    def flush_chunk() -> None:
        if not chunk:
            return
        fragment = escape_drat_string("".join(chunk))
        lines.append(f'    content = str_concat(content, "{fragment}")')
        chunk.clear()

    for ch in content:
        if ch == "\n":
            flush_chunk()
            lines.append("    content = str_concat(content, ascii_char(10))")
        elif ch == "\r":
            flush_chunk()
            lines.append("    content = str_concat(content, ascii_char(13))")
        elif ch == "\t":
            flush_chunk()
            lines.append("    content = str_concat(content, ascii_char(9))")
        else:
            chunk.append(ch)

    flush_chunk()
    return "\n".join(lines)


def rewrite_demo_source(original: str, case_name: str, content: str) -> str:
    escaped_name = escape_drat_string(case_name)
    content_builder = build_content_statements(content)
    replacement = (
        "fn demo_source_file() {\n"
        f"{content_builder}\n"
        f'    make_source_file("{escaped_name}", content)\n'
        "}"
    )
    updated, count = re.subn(
        r"fn demo_source_file\(\) \{\n(?:.*\n)*?\}",
        lambda _: replacement,
        original,
        count=1,
    )
    if count != 1:
        raise RuntimeError("khong the cap nhat demo_source_file trong src/source/file.dt")
    return updated


def ensure_host_drat() -> None:
    build = run(["cargo", "build", "-p", "drat"])
    if build.returncode != 0:
        raise RuntimeError(build.stderr or build.stdout)


def verify_case(case_name: str, content: str, original_source_file: str) -> tuple[bool, str]:
    patched = rewrite_demo_source(original_source_file, case_name, content)
    SOURCE_FILE.write_text(patched)
    TEMP_CASE.write_text(content)

    build_env = {
        **dict(**os.environ),
        "DRATON_DISABLE_GCROOT": "1",
        "DRATON_ALLOW_MULTIPLE_RUNTIME_DEFS": "1",
    }
    build = run([str(DRAT), "build"], env=build_env)
    if build.returncode != 0:
        return False, f"build failed\n{build.stdout}{build.stderr}"

    rust = run([str(DRAT), "lex-dump", str(TEMP_CASE)])
    if rust.returncode != 0:
        return False, f"rust lex-dump failed\n{rust.stdout}{rust.stderr}"

    selfhost = run([str(SELFHOST)])
    if selfhost.returncode != 0:
        return False, f"self-host run failed\n{selfhost.stdout}{selfhost.stderr}"

    if rust.stdout != selfhost.stdout:
        return (
            False,
            "stdout mismatch\n"
            f"rust    = {rust.stdout!r}\n"
            f"selfhost= {selfhost.stdout!r}\n",
        )

    return True, rust.stdout


def main() -> int:
    ensure_host_drat()
    original_source_file = SOURCE_FILE.read_text()
    lines = [
        "Draton self-host Phase 1 corpus verification",
        "Date: 2026-03-12",
        "",
        f"Cases: {len(CASES)}",
        "",
    ]
    failures = 0

    try:
        for case_name, content in CASES:
            ok, detail = verify_case(case_name, content, original_source_file)
            status = "PASS" if ok else "FAIL"
            lines.append(f"[{status}] {case_name}")
            lines.append(f"source: {content!r}")
            if ok:
                lines.append(f"stdout: {detail!r}")
            else:
                lines.append(detail.rstrip())
                failures += 1
            lines.append("")
    finally:
        SOURCE_FILE.write_text(original_source_file)
        if TEMP_CASE.exists():
            TEMP_CASE.unlink()

    RESULT_FILE.write_text("\n".join(lines).rstrip() + "\n")
    print(RESULT_FILE)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
