# Tooling map

## Local tool directory

- [`.codex/tools/run_guarded.py`](../../../../.codex/tools/run_guarded.py)
- [`.codex/tools/list_tools.py`](../../../../.codex/tools/list_tools.py)
- [`.codex/tools/cleanup_tool_state.py`](../../../../.codex/tools/cleanup_tool_state.py)

## When to use `run_guarded.py`

Use it for:

- `cargo build`
- `cargo test`
- release packaging or smoke tests
- long-running Python scripts
- tasks that could otherwise spawn many expensive processes

Recommended defaults:

- timeout: 900s
- wait for slot: 120s
- concurrency: 2
- memory: 2048 MB
- CPU time: 600s

Example:

```bash
python3 .codex/tools/run_guarded.py --timeout-sec 1200 --memory-mb 3072 -- cargo test --workspace
```

## Recovery

If a guarded run is interrupted, clean stale slot state with:

```bash
python3 .codex/tools/cleanup_tool_state.py
```
