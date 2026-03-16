# Draton Quickstart

This quickstart assumes you installed a prebuilt Early Tooling Preview release.

The release installers update `PATH` automatically when practical. Open a new shell after installation if `drat` is not visible immediately.

## 1. Verify the CLI

```sh
drat --version
```

## 2. Run the bundled hello-world file

```sh
drat run examples/hello.dt
```

Expected output:

```text
hello, draton!
```

## 3. Use the bundled sample project

```sh
cd examples/early-preview/hello-app
```

## 4. Check formatting

```sh
drat fmt --check src
```

## 5. Run the linter

```sh
drat lint src
```

## 6. Inspect tasks

```sh
drat task
```

## 7. Build via the task runner

```sh
drat task build
```

Run the built program:

Linux / macOS:

```sh
./build/hello-preview
```

Windows PowerShell:

```powershell
.\build\hello-preview
```

## 8. Build and run the sample project directly

Still inside `examples/early-preview/hello-app`:

```sh
drat build
drat run
```

## 9. Build directly with `drat`

From the repository root or extracted archive root:

```sh
drat build examples/hello.dt -o hello-tooling
```

Then run the output:

Linux / macOS:

```sh
./hello-tooling
```

Windows:

```powershell
.\hello-tooling
```

## 10. Start the language server

```sh
drat lsp
```

Your editor should launch that command as a stdio language server.

## Need full install details?

See:

- [install.md](install.md)
- [early-preview.md](early-preview.md)
