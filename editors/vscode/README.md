# Draton for Visual Studio Code

<div align="center">

[![Version](https://img.shields.io/visual-studio-marketplace/v/draton.draton?style=flat-square)](https://marketplace.visualstudio.com/items?itemName=draton.draton)
[![Installs](https://img.shields.io/visual-studio-marketplace/i/draton.draton?style=flat-square)](https://marketplace.visualstudio.com/items?itemName=draton.draton)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square)](https://github.com/draton-lang/draton/blob/main/LICENSE)

</div>

Official VS Code extension for the [Draton programming language](https://github.com/draton-lang/draton). Provides syntax highlighting, real-time diagnostics via the Language Server Protocol, and the bundled **Draton Prism Night** dark theme.

---

## Features

### Syntax Highlighting

Full TextMate grammar covering the Draton canonical syntax:

- Keywords: `let`, `mut`, `fn`, `pub`, `class`, `interface`, `enum`, `match`, `spawn`, `return`, `import`
- Type annotations: `@type` blocks, `Result[T, E]`, `chan[T]`
- String interpolation: `f"Hello, {name}!"`
- Low-level blocks: `unsafe`, `@pointer`, `@comptime`, inline `asm`
- Comments, operators, and literals

### Language Server (LSP)

When `drat` or `draton-lsp` is available on `PATH` or in your project's `target/` directory, the extension activates a full LSP client providing:

- **Diagnostics** — type errors and syntax issues shown inline
- **Hover** — type information on hover
- **Go to Definition** — navigate to symbol definitions
- **Document Symbols** — outline view with functions, classes, and enums
- **Completion** — basic identifier completion

### Draton Prism Night Theme

A built-in dark color theme tuned for Draton syntax. Activate it via **Preferences → Color Theme → Draton Prism Night**.

---

## Requirements

- **VS Code** `1.75.0` or later

For LSP features, install the Draton compiler (`drat`) from the [GitHub Releases](https://github.com/draton-lang/draton/releases) page or build from source. The extension works without `drat` — syntax highlighting and the theme are always available.

---

## Installation

### From the Marketplace

Search for **Draton** in the Extensions view (`Ctrl+Shift+X` / `Cmd+Shift+X`) and click **Install**.

### From a VSIX file

1. Download the latest `.vsix` from [GitHub Releases](https://github.com/draton-lang/draton/releases).
2. Open the Command Palette (`Ctrl+Shift+P`), run **Extensions: Install from VSIX...**, and select the file.

Or via the terminal:

```sh
code --install-extension draton.draton-<version>.vsix
```

---

## Extension Settings

| Setting | Default | Description |
|---|---|---|
| `draton.server.path` | `""` | Path to the `drat` or `draton-lsp` binary. Leave empty to use automatic resolution. |
| `draton.server.args` | `[]` | Additional arguments passed to the language server on startup. |

### Language server resolution order

When `draton.server.path` is empty the extension searches for the language server in this order:

1. `target/debug/drat`
2. `target/release/drat`
3. `target/debug/draton-lsp`
4. `target/release/draton-lsp`
5. `drat` on `PATH`
6. `draton-lsp` on `PATH`

---

## Quick Start

```sh
# Install Draton (Linux / macOS)
curl -fsSL https://github.com/draton-lang/draton/releases/latest/download/install.sh | sh

# Install Draton (Windows PowerShell)
irm https://github.com/draton-lang/draton/releases/latest/download/install.ps1 | iex

# Verify
drat --version
```

Then open any `.dt` file — syntax highlighting activates immediately. If `drat` is on your `PATH`, the language server starts automatically when you open a Draton file.

---

## Known Limitations

- LSP features require `drat` or `draton-lsp` to be installed separately.
- Windows `aarch64` is not supported in the current Draton Early Tooling Preview.

---

## Contributing

Bug reports, feature requests, and pull requests are welcome. Please read [CONTRIBUTING](https://github.com/draton-lang/draton/blob/main/.github/CONTRIBUTING.md) before opening a PR.

## License

Apache License, Version 2.0. See [LICENSE](https://github.com/draton-lang/draton/blob/main/LICENSE) for details.
