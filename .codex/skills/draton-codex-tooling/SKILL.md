---
name: draton-codex-tooling
description: Choose and use Draton repository-local Codex tools for safe command execution and tool discovery. Use when Codex needs to run potentially expensive commands, constrain time or memory, avoid spawning too many concurrent processes, recover from interrupted guarded runs, or decide whether local tools should replace raw shell execution.
---

# Draton Codex Tooling

Default to repository-local guarded tools for command execution. The goal is to prevent Codex from flooding the machine with overlapping builds, tests, or long-running scripts.

## Workflow

1. Read [references/tooling-map.md](references/tooling-map.md).
2. If the task needs command execution beyond a trivial quick read, use a local tool before considering raw shell execution.
3. If the task is generic but potentially expensive, prefer `.codex/tools/run_guarded.py`.
4. If the task is a common cargo workflow, prefer `.codex/tools/guarded_cargo.py`.
5. Use `.codex/tools/system_snapshot.py` before expensive verification when host pressure may matter.
6. Use `.codex/tools/repo_processes.py` to inspect overlapping repo jobs.
7. Use `.codex/tools/stop_repo_processes.py` only when repo jobs need controlled shutdown.
8. Use `.codex/tools/list_tools.py` when the best local tool is unclear.
9. Use `.codex/tools/cleanup_tool_state.py` after interrupted runs or stale slot state.
10. Coordinate with `$draton-verification`, `$draton-release-readiness`, or `$draton-vendored-llvm` for the actual command list.

## Rules

- Default to local tools instead of raw shell for non-trivial execution.
- Prefer one guarded command over multiple uncontrolled background commands.
- Keep concurrency low unless the task truly benefits from more slots.
- Set time and memory budgets intentionally for heavy builds and tests.
- Snapshot the machine before heavy work if there is any sign of resource pressure.
- Inspect and stop repo-local processes before escalating to broader kill patterns.
- Treat raw shell execution as the fallback, not the default, for non-trivial work.

## Resources

- Load [references/tooling-map.md](references/tooling-map.md) for local tool usage guidance.
