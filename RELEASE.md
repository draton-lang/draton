# Releasing Draton Early Tooling Preview

This repository ships end-user Early Tooling Preview archives through GitHub Actions.

## Maintainer Checklist

1. Ensure `main` is green.
2. Confirm version strings and changelog context are ready.
3. Run a local sanity check:

   ```sh
   cargo build --release -p drat -p draton-runtime -p draton-lsp
   python3 scripts/package_release.py \
      --binary target/release/drat \
      --artifact draton-early-linux-x86_64.tar.gz \
      --out-dir dist
   python3 scripts/smoke_release.py --archive dist/draton-early-linux-x86_64.tar.gz
   ```

4. Create and push a release tag:

   ```sh
   git tag v0.X.Y
   git push origin v0.X.Y
   ```

5. Watch `.github/workflows/release.yml` in GitHub Actions until all jobs finish.
6. Open the GitHub Release page and confirm these assets exist:
   - `draton-early-linux-x86_64.tar.gz`
   - `draton-early-linux-aarch64.tar.gz`
   - `draton-early-macos-x86_64.tar.gz`
   - `draton-early-macos-aarch64.tar.gz`
   - `draton-early-windows-x86_64.zip`
   - `install.sh`
   - `install.ps1`
   - matching `.sha256` files
   - `SHA256SUMS.txt`
7. Verify the generated release notes and edit them if needed.

## Exact Release Command

```sh
git tag v0.X.Y
git push origin v0.X.Y
```

That tag push is the maintainer action that cuts an Early Tooling Preview release.

## What Ships

Every shipped preview archive contains:

- `drat`
- the packaged Draton runtime static library
- `LICENSE`
- `QUICKSTART.md`
- `INSTALL.md`
- `EARLY-PREVIEW.md`
- `examples/hello.dt`
- `examples/early-preview/hello-app/`
- `install.sh`
- `install.ps1`

## Current Limitations

- Binaries are unsigned.
- macOS builds are not notarized.
- Linux preview binaries still rely on standard system runtime libraries such as `libstdc++`, `libffi`, `libz`, and `libtinfo`; they do not require a separate LLVM install.
- Windows aarch64 is not part of the current Early Tooling Preview target set.
- No package-manager distribution yet.

## Follow-Up Improvements

- Code signing for Windows binaries.
- macOS signing and notarization.
- Homebrew, Scoop, winget, apt, pacman packages.
- Native installers such as `.msi` and `.pkg`.
