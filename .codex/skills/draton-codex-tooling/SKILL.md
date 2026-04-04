---
name: draton-codex-tooling
description: Choose and use Draton repository-local Codex tools for safe command execution and tool discovery. Use when Codex needs to run potentially expensive commands, constrain time or memory, avoid spawning too many concurrent processes, recover from interrupted guarded runs, or decide whether local tools should replace raw shell execution.
---

# Draton Codex Tooling

Prefer repository-local guarded tools when command execution strategy matters. The goal is to prevent Codex from flooding the machine with overlapping builds, tests, or long-running scripts.

## Workflow

1. Read [references/tooling-map.md](references/tooling-map.md).
2. If the task needs command execution beyond a trivial quick read, prefer `.codex/tools/run_guarded.py`.
3. Use `.codex/tools/list_tools.py` when the best local tool is unclear.
4. Use `.codex/tools/cleanup_tool_state.py` after interrupted runs or stale slot state.
5. Coordinate with `$draton-verification`, `$draton-release-readiness`, or `$draton-vendored-llvm` for the actual command list.

## Rules

- Prefer one guarded command over multiple uncontrolled background commands.
- Keep concurrency low unless the task truly benefits from more slots.
- Set time and memory budgets intentionally for heavy builds and tests.
- Treat raw shell execution as the fallback, not the default, for expensive work.

## Resources

- Load [references/tooling-map.md](references/tooling-map.md) for local tool usage guidance.
