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

- none inside the supported Early Tooling Preview matrix

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

Prebuilt Early Tooling Preview archives do not require a separate LLVM install on supported targets.

What they do rely on:

- Linux: standard runtime libraries normally present on mainstream distributions, especially `glibc`, `libstdc++`, `libffi`, `zlib`, and `libtinfo`
- macOS: system runtime libraries that ship with supported macOS releases
- Windows x86_64: the standard Windows user-mode runtime stack present on supported desktop systems

If you are building Draton from source instead of using a release archive, LLVM 14 development libraries are still required.

## Install with scripts

### Linux and macOS

Use the release-hosted installer:

```sh
curl -fsSL https://github.com/draton-lang/draton/releases/download/v0.1.41/install.sh | sh
```

Pin a specific tag:

```sh
curl -fsSL https://github.com/draton-lang/draton/releases/download/v0.1.41/install.sh | sh -s -- --version v0.1.41
```

Default install location:

- payload: `~/.local/share/draton/current`
- add to `PATH`: `~/.local/share/draton/current`
- installer behavior: appends that directory to the first writable profile it finds (`~/.bashrc`, `~/.zshrc`, or `~/.profile`)

The installer verifies success with:

```sh
drat --version
```

The installer also verifies the downloaded archive against the published `.sha256` checksum before extracting it.

### Windows x86_64

Use PowerShell:

```powershell
Invoke-WebRequest `
  -Uri https://github.com/draton-lang/draton/releases/download/v0.1.41/install.ps1 `
  -OutFile install.ps1
.\install.ps1 -Version v0.1.41
```

Default install location:

- payload: `%LOCALAPPDATA%\Draton\current`
- add to `PATH`: `%LOCALAPPDATA%\Draton\current`
- installer behavior: appends that directory to the current user's `Path` environment variable when needed

The script verifies:

```powershell
drat --version
```

The script also verifies the downloaded archive against the published `.sha256` checksum before extracting it.

### Windows aarch64

Not part of the current Early Tooling Preview target set.

Current status:

- no published release artifact
- no official install script path
- not claimed as supported functionality in this preview

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

Prebuilt preview binaries should not require a separate LLVM install. If startup still fails, the usual cause is a missing standard system runtime library on a minimal machine image.

Typical Linux fixes:

- Debian / Ubuntu: `sudo apt install libstdc++6 libffi8 zlib1g libtinfo6`
- Fedora: install the matching `libstdc++`, `libffi`, `zlib`, and `ncurses-compat-libs` packages
- Arch: ensure `gcc-libs`, `libffi`, `zlib`, and `ncurses` are present

On macOS, unsigned-binary or Gatekeeper restrictions are more likely than a missing runtime library.

On Windows x86_64, verify that you are on a normal desktop/runtime image and not a stripped-down environment.

### `drat build` fails from an installed archive

Check that:

- the runtime static library still sits beside `drat`
- you did not move only the executable out of the extracted directory
- `drat --version` works first

### macOS unsigned binary warning

The Early Tooling Preview is unsigned and not notarized yet. Remove the quarantine attribute manually if needed.
