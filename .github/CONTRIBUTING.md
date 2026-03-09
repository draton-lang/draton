# Contributing to Draton

Thanks for your interest in contributing to Draton.

## Before You Start

- Read the [Code of Conduct](CODE_OF_CONDUCT.md)
- Search existing issues and pull requests before opening a new one
- For security issues, do not open a public issue; follow [SECURITY.md](SECURITY.md)

## Development Setup

1. Install the Rust stable toolchain.
2. Install LLVM 14 and make sure `llvm-config` resolves to that version.
3. Clone the repository and enter the workspace root.

```bash
git clone git@github.com:draton-lang/draton.git
cd draton
```

## Recommended Workflow

1. Create a branch from `main`.
2. Make one logical change at a time.
3. Add or update tests with the change.
4. Run formatting, tests, and lint checks locally.
5. Open a pull request with a clear description of what changed and why.

## Local Verification

Run these commands before opening a pull request:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

If you are changing only one crate, you can run targeted commands first, but the full
workspace should still pass before merge.

## Commit Messages

Use clear, descriptive commit messages. A good default format is:

```text
<type>: <short summary>
```

Examples:

- `feat: add class layer syntax`
- `fix: preserve spans for multiline comments`
- `refactor: simplify method predeclaration in type checker`

Avoid vague messages like `fix`, `update`, or `misc`.

## Pull Request Expectations

Please include:

- A concise summary of the problem
- The approach you took
- Any user-visible or language-level behavior changes
- Tests that cover the change
- Follow-up work, if anything remains intentionally out of scope

Small, focused pull requests are much easier to review than broad mixed changes.

## Style Notes

- Keep code changes consistent with the surrounding crate style
- Prefer small, targeted patches over broad rewrites
- Preserve compatibility unless the change explicitly intends to evolve the language or
  public API
- Do not leave TODO placeholders in code submitted for review

## Reporting Bugs

Use the issue templates in this repository where possible. Include reproduction steps,
expected behavior, actual behavior, and environment details.

## Questions

If you are unsure whether a change belongs in the language, runtime, or CLI, open an
issue first so the design can be discussed before implementation.
