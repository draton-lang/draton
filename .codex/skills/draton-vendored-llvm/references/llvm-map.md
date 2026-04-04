# Vendored LLVM map

## Core workflow

- Fetch LLVM with [`scripts/vendor_llvm.py`](../../../../scripts/vendor_llvm.py).
- Print the environment with `scripts/vendor_llvm.py print-env --target host`.
- Build after the vendored environment is active.
- If packaging is involved, keep bundle handling aligned with release smoke coverage.

## Related files

- [`scripts/vendor_llvm.py`](../../../../scripts/vendor_llvm.py)
- [`scripts/package_release.py`](../../../../scripts/package_release.py)
- [`scripts/smoke_release.py`](../../../../scripts/smoke_release.py)
- [`docs/release-workflow.md`](../../../../docs/release-workflow.md)

## Rule of thumb

Prefer explicit vendored-toolchain commands over undocumented local machine assumptions.
