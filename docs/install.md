# Install Draton

Draton ships prebuilt `drat` archives for:

- Linux x86_64
- Linux aarch64
- macOS x86_64
- macOS aarch64
- Windows x86_64

Each archive includes:

- `drat`
- the packaged Draton runtime static library
- `LICENSE`
- `QUICKSTART.md`
- `examples/hello.dt`

## Runtime Prerequisite

Current releases are built against LLVM 14. End users need the LLVM 14 runtime libraries available on their system before `drat` will start.

Typical packages:

- Debian / Ubuntu: `llvm-14` or `libllvm14`
- Fedora: `llvm` 14 runtime package
- Arch: `llvm14-libs`
- macOS Homebrew: `brew install llvm@14`
- Windows: install LLVM 14 and add its `bin` directory to `PATH`

If `drat --version` fails with a missing LLVM shared library error, install LLVM 14 first and retry.

## Download

Release assets live on the GitHub Releases page for the tagged version:

`https://github.com/draton-lang/draton/releases`

Artifact names:

- `draton-linux-x86_64.tar.gz`
- `draton-linux-aarch64.tar.gz`
- `draton-macos-x86_64.tar.gz`
- `draton-macos-aarch64.tar.gz`
- `draton-windows-x86_64.zip`
- `SHA256SUMS.txt`

## Linux

### x86_64

```sh
curl -L -o draton-linux-x86_64.tar.gz \
  https://github.com/draton-lang/draton/releases/download/vX.Y.Z/draton-linux-x86_64.tar.gz
tar -xzf draton-linux-x86_64.tar.gz
export PATH="$PWD/draton-linux-x86_64:$PATH"
drat --version
drat run examples/hello.dt
```

### aarch64

```sh
curl -L -o draton-linux-aarch64.tar.gz \
  https://github.com/draton-lang/draton/releases/download/vX.Y.Z/draton-linux-aarch64.tar.gz
tar -xzf draton-linux-aarch64.tar.gz
export PATH="$PWD/draton-linux-aarch64:$PATH"
drat --version
drat run examples/hello.dt
```

To persist `PATH`, append the export line to `~/.bashrc`, `~/.zshrc`, or your shell profile.

## macOS

### Intel

```sh
curl -L -o draton-macos-x86_64.tar.gz \
  https://github.com/draton-lang/draton/releases/download/vX.Y.Z/draton-macos-x86_64.tar.gz
tar -xzf draton-macos-x86_64.tar.gz
export PATH="$PWD/draton-macos-x86_64:$PATH"
drat --version
drat run examples/hello.dt
```

### Apple Silicon

```sh
curl -L -o draton-macos-aarch64.tar.gz \
  https://github.com/draton-lang/draton/releases/download/vX.Y.Z/draton-macos-aarch64.tar.gz
tar -xzf draton-macos-aarch64.tar.gz
export PATH="$PWD/draton-macos-aarch64:$PATH"
drat --version
drat run examples/hello.dt
```

If Gatekeeper warns about an unsigned binary, allow it explicitly:

```sh
xattr -d com.apple.quarantine drat-macos-*/drat 2>/dev/null || true
```

These binaries are unsigned and not notarized yet.

## Windows

PowerShell:

```powershell
Invoke-WebRequest `
  -Uri https://github.com/draton-lang/draton/releases/download/vX.Y.Z/draton-windows-x86_64.zip `
  -OutFile draton-windows-x86_64.zip
Expand-Archive draton-windows-x86_64.zip -DestinationPath .
$env:Path = "$PWD\\draton-windows-x86_64;$env:Path"
drat --version
drat run examples/hello.dt
```

To keep it on `PATH`, add the extracted directory to your user or system environment variables.

## 30-Second Quickstart

After extraction:

```sh
drat --version
drat run examples/hello.dt
```

Expected output:

```text
hello, draton!
```

## Verify Checksums

Download `SHA256SUMS.txt` and verify the archive before extracting it.

Linux / macOS:

```sh
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS with BSD tools only:

```sh
shasum -a 256 draton-macos-aarch64.tar.gz
```

Windows PowerShell:

```powershell
Get-FileHash .\draton-windows-x86_64.zip -Algorithm SHA256
```

Compare the printed hash against `SHA256SUMS.txt` or the matching `.sha256` asset.

## Troubleshooting

### `drat` does not start

Install LLVM 14 runtime libraries and retry.

Typical errors:

- Linux: missing `libLLVM-14.so`
- macOS: missing `libLLVM.dylib`
- Windows: missing `LLVM-C.dll` or other LLVM DLLs

### `drat run examples/hello.dt` fails

Check:

- you extracted the full archive
- `examples/hello.dt` is present next to the binary directory
- `drat --version` succeeds first

### Permission denied on Unix

If the executable bit was stripped by your extraction tool:

```sh
chmod +x draton-*/drat
```
