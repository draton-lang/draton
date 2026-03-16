# Install Draton Early Tooling Preview

This document covers end-user installation for the first public Draton Early Tooling Preview.

## What ships

Supported preview archives:

- `draton-early-linux-x86_64.tar.gz`
- `draton-early-linux-aarch64.tar.gz`
- `draton-early-macos-x86_64.tar.gz`
- `draton-early-macos-aarch64.tar.gz`
- `draton-early-windows-x86_64.zip`

Currently blocked:

- `Windows aarch64`

The Windows aarch64 blocker is explicit: the current LLVM 14 + `inkwell` toolchain path is not yet verified for producing and testing a release-quality `aarch64-pc-windows-msvc` CLI on GitHub-hosted infrastructure. Draton does not claim that target until it is verified.

Every supported archive contains:

- `drat` or `drat.exe`
- packaged Draton runtime static library
- `LICENSE`
- `QUICKSTART.md`
- `INSTALL.md`
- `EARLY-PREVIEW.md`
- `examples/hello.dt`
- `examples/early-preview/hello-app/`
- `install.sh`
- `install.ps1`

## Runtime prerequisite

Current preview builds require LLVM 14 runtime libraries on the target machine.

Typical packages:

- Debian / Ubuntu: `llvm-14` or `libllvm14`
- Fedora: LLVM 14 runtime package
- Arch: `llvm14-libs`
- macOS Homebrew: `brew install llvm@14`
- Windows x86_64: install LLVM 14 and add its `bin` directory to `PATH`

If `drat --version` fails with a missing LLVM shared library error, install LLVM 14 first and retry.

## Install with scripts

### Linux and macOS

Use the release-hosted installer:

```sh
curl -fsSL https://github.com/draton-lang/draton/releases/download/v0.X.Y/install.sh | sh
```

Pin a specific tag:

```sh
curl -fsSL https://github.com/draton-lang/draton/releases/download/v0.X.Y/install.sh | sh -s -- --version v0.X.Y
```

Default install location:

- payload: `~/.local/share/draton/current`
- add to `PATH`: `~/.local/share/draton/current`

The installer verifies success with:

```sh
drat --version
```

### Windows x86_64

Use PowerShell:

```powershell
Invoke-WebRequest `
  -Uri https://github.com/draton-lang/draton/releases/download/v0.X.Y/install.ps1 `
  -OutFile install.ps1
.\install.ps1 -Version v0.X.Y
```

Default install location:

- payload: `%LOCALAPPDATA%\Draton\current`
- add to `PATH`: `%LOCALAPPDATA%\Draton\current`

The script verifies:

```powershell
drat --version
```

### Windows aarch64

Not published in the Early Tooling Preview.

Current status:

- no verified release artifact
- no official install script path
- tracked as a release-engineering blocker, not as supported functionality

## Manual install

### Linux x86_64

```sh
curl -L -o draton-early-linux-x86_64.tar.gz \
  https://github.com/draton-lang/draton/releases/download/v0.X.Y/draton-early-linux-x86_64.tar.gz
tar -xzf draton-early-linux-x86_64.tar.gz
export PATH="$PWD/draton-early-linux-x86_64:$PATH"
drat --version
```

### Linux aarch64

```sh
curl -L -o draton-early-linux-aarch64.tar.gz \
  https://github.com/draton-lang/draton/releases/download/v0.X.Y/draton-early-linux-aarch64.tar.gz
tar -xzf draton-early-linux-aarch64.tar.gz
export PATH="$PWD/draton-early-linux-aarch64:$PATH"
drat --version
```

### macOS Intel

```sh
curl -L -o draton-early-macos-x86_64.tar.gz \
  https://github.com/draton-lang/draton/releases/download/v0.X.Y/draton-early-macos-x86_64.tar.gz
tar -xzf draton-early-macos-x86_64.tar.gz
export PATH="$PWD/draton-early-macos-x86_64:$PATH"
drat --version
```

### macOS Apple Silicon

```sh
curl -L -o draton-early-macos-aarch64.tar.gz \
  https://github.com/draton-lang/draton/releases/download/v0.X.Y/draton-early-macos-aarch64.tar.gz
tar -xzf draton-early-macos-aarch64.tar.gz
export PATH="$PWD/draton-early-macos-aarch64:$PATH"
drat --version
```

If Gatekeeper warns about an unsigned binary:

```sh
xattr -d com.apple.quarantine draton-early-macos-*/drat 2>/dev/null || true
```

### Windows x86_64

```powershell
Invoke-WebRequest `
  -Uri https://github.com/draton-lang/draton/releases/download/v0.X.Y/draton-early-windows-x86_64.zip `
  -OutFile draton-early-windows-x86_64.zip
Expand-Archive draton-early-windows-x86_64.zip -DestinationPath .
$env:Path = "$PWD\\draton-early-windows-x86_64;$env:Path"
drat --version
```

## PATH setup

Recommended persistent PATH entries:

- Linux / macOS script installs: `~/.local/share/draton/current`
- Windows script installs: `%LOCALAPPDATA%\Draton\current`
- Manual installs: the extracted archive root directory

## Checksum verification

Download `SHA256SUMS.txt` plus the matching archive or `.sha256` file.

Linux:

```sh
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS:

```sh
shasum -a 256 draton-early-macos-aarch64.tar.gz
```

Windows:

```powershell
Get-FileHash .\draton-early-windows-x86_64.zip -Algorithm SHA256
```

Compare the printed hash against `SHA256SUMS.txt` or the per-asset `.sha256` file.

## Uninstall

Script installs:

- Linux / macOS: remove `~/.local/share/draton`
- Windows: remove `%LOCALAPPDATA%\Draton`
- then remove the corresponding PATH entry if you added one

Manual installs:

- delete the extracted directory
- remove its PATH entry

## Troubleshooting

### `drat --version` fails

Install LLVM 14 runtime libraries first.

Typical failures:

- Linux: missing `libLLVM-14.so`
- macOS: missing `libLLVM.dylib`
- Windows: missing LLVM DLLs from the LLVM 14 install

### `drat build` fails from an installed archive

Check that:

- the runtime static library still sits beside `drat`
- you did not move only the executable out of the extracted directory
- `drat --version` works first

### macOS unsigned binary warning

The Early Tooling Preview is unsigned and not notarized yet. Remove the quarantine attribute manually if needed.
