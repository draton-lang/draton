# Releasing Draton

This repository ships end-user release archives through GitHub Actions.

## Maintainer Checklist

1. Ensure `main` is green.
2. Confirm version strings and changelog context are ready.
3. Run a local sanity check:

   ```sh
   cargo build -p drat -p draton-runtime
   python3 scripts/package_release.py \
     --binary target/debug/drat \
     --artifact draton-linux-x86_64.tar.gz \
     --out-dir dist
   python3 scripts/smoke_release.py --archive dist/draton-linux-x86_64.tar.gz
   ```

4. Create and push a release tag:

   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

5. Watch `.github/workflows/release.yml` in GitHub Actions until all jobs finish.
6. Open the GitHub Release page and confirm these assets exist:
   - `draton-linux-x86_64.tar.gz`
   - `draton-linux-aarch64.tar.gz`
   - `draton-macos-x86_64.tar.gz`
   - `draton-macos-aarch64.tar.gz`
   - `draton-windows-x86_64.zip`
   - matching `.sha256` files
   - `SHA256SUMS.txt`
7. Verify the generated release notes and edit them if needed.

## Exact Release Command

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

That tag push is the maintainer action that cuts a release.

## What Ships

Every archive contains:

- `drat`
- the packaged Draton runtime static library
- `LICENSE`
- `QUICKSTART.md`
- `examples/hello.dt`

## Current Limitations

- Binaries are unsigned.
- macOS builds are not notarized.
- Users still need the LLVM 14 runtime installed on their machine.
- No package-manager distribution yet.

## Follow-Up Improvements

- Code signing for Windows binaries.
- macOS signing and notarization.
- Homebrew, Scoop, winget, apt, pacman packages.
- Native installers such as `.msi` and `.pkg`.
