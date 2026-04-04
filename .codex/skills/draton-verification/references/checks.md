# Verification map

## Area to command mapping

- `parser`
  - `cargo test -p draton-parser --test items`
- `typeck`
  - `cargo test -p draton-typeck --test errors`
- `workspace`
  - `cargo test --workspace`
- `fmt`
  - `cargo fmt --all --check`
- `clippy`
  - `cargo clippy --workspace -- -D warnings`
- `release`
  - run the most relevant `scripts/package_release.py` or `scripts/smoke_release.py` path affected by the change
- `llvm`
  - validate the matching `scripts/vendor_llvm.py` flow and any impacted build or smoke command
- `docs`
  - perform a consistency pass between changed docs and the implementation they describe

## Default approach

1. Start narrow.
2. Expand only if the change surface requires it.
3. Rerun after fixes.
4. Report exact commands and outcomes.
