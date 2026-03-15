# Draton Quickstart

1. Extract the archive for your platform.
2. Add the extracted directory to `PATH`.
3. Verify the CLI:

```sh
drat --version
```

4. Run the bundled hello-world example:

```sh
drat run examples/hello.dt
```

Expected output:

```text
hello, draton!
```

The archive also includes the Draton runtime static library required for `drat build` and `drat run`.

If `drat` fails to start with a missing LLVM library error, install the LLVM 14 runtime for your platform first. See `docs/install.md` for platform-specific instructions.
