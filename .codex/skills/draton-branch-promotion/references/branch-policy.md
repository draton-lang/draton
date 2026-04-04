# Branch policy

Mirror this policy exactly:

- `dev` is the active development branch for ongoing implementation and frequent pushes.
- `unstable` is the broader validation and release-candidate branch that receives promoted code from `dev`.
- `main` is the stable branch and should only receive code already proven stable.

Promotion path:

```text
dev -> unstable -> main
```

## Sync points

When branch policy text changes, keep these aligned:

- [`AGENTS.md`](../../../../AGENTS.md)
- [`.github/CONTRIBUTING.md`](../../../../.github/CONTRIBUTING.md)
- [`README.md`](../../../../README.md)
- [`docs/release-workflow.md`](../../../../docs/release-workflow.md)
