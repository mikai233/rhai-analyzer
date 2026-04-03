<p align="center">
  <img src="./icon.png" width="128" alt="Rhai Analyzer icon" />
</p>

# Rhai Analyzer

Rhai language support for Visual Studio Code, powered by the `rhai-analyzer` language server.

## Overview

Rhai Analyzer brings editor tooling for the Rhai scripting language to Visual Studio Code. The extension starts and manages `rhai-lsp`, contributes Rhai language assets to VS Code, and exposes language features backed by the Rust analysis engine.

The extension is designed to stay aligned with real Rhai syntax and semantics rather than a simplified editor-specific dialect.

## Features

- Semantic diagnostics for Rhai source files
- Hover information and signature help
- Go to definition, find references, and rename
- Code completion, including project and module symbols
- Inlay hints for variables, parameters, and return types
- Document formatting powered by the shared `rhai-fmt` formatter
- Semantic tokens, document symbols, workspace symbols, and folding ranges
- Call hierarchy, document highlights, and code actions
- Baseline syntax highlighting, Rhai snippets, and Markdown fenced-code highlighting

## Getting Started

The extension activates automatically for `.rhai` files.

In a packaged installation, the extension looks for a bundled `rhai-lsp` binary inside the extension. For local development or custom deployments, you can also point the extension at an explicit server executable through `rhai.server.path`.

## Extension Settings

The extension contributes the following settings under the `rhai` namespace:

| Setting | Description |
| --- | --- |
| `rhai.server.path` | Absolute path to a custom `rhai-lsp` executable. |
| `rhai.server.transport` | Transport used to communicate with the language server: `stdio` or `tcp`. |
| `rhai.server.tcpAddress` | TCP endpoint used when `rhai.server.transport` is set to `tcp`. |
| `rhai.server.logLevel` | Log level passed to `rhai-lsp` when the extension launches it. |
| `rhai.trace.server` | VS Code LSP trace level for protocol inspection. |
| `rhai.inlayHints.variables` | Show inferred type hints for local variables. |
| `rhai.inlayHints.parameters` | Show inferred type hints for function and closure parameters. |
| `rhai.inlayHints.returnTypes` | Show inferred return type hints for functions and closures. |
| `rhai.format.maxLineLength` | Preferred maximum line length for formatting. |
| `rhai.format.trailingCommas` | Preserve trailing commas in expanded lists and containers. |
| `rhai.format.finalNewline` | Ensure formatted files end with a trailing newline. |
| `rhai.format.containerLayout` | Prefer single-line, multi-line, or automatic container layout decisions. |
| `rhai.format.importSortOrder` | Preserve source order or sort contiguous import sections by module path. |

Project-level formatter configuration can also be provided through `rhai.toml`. The shared formatter configuration surface is documented in the workspace `RHAI_TOML.md` file.

## Development

1. Build the language server:

   ```powershell
   cargo build -p rhai-lsp
   ```

2. Install dependencies and build the extension:

   ```powershell
   cd clients/vscode
   npm install
   npm run build
   ```

3. Open `clients/vscode` in Visual Studio Code and press `F5`.

The extension host launch configuration rebuilds both the VS Code client bundle and `rhai-lsp`.

## Packaging

To produce a local VSIX package and build the release `rhai-lsp` binary as part of the process:

```powershell
cd clients/vscode
npm install
npm run package:local
```

For CI and other prebuilt packaging flows, stage the server bundle first and package it afterwards:

```powershell
cd clients/vscode
npm run prepare-server
npm run package
```

The `package` script only packages the currently staged `clients/vscode/server` bundle and will not rebuild or restage `rhai-lsp`.

This split avoids overwriting a preassembled multi-target server bundle during the packaging step. It is especially important in CI, where multiple platform binaries may already have been downloaded into a staging directory before packaging.

To clear generated extension artifacts and staged server bundles:

```powershell
cd clients/vscode
npm run clean
```

The packaged extension is written to:

```text
clients/vscode/.artifacts/rhai-analyzer.vsix
```

You can then install it through the Visual Studio Code command:

`Extensions: Install from VSIX...`

## Status

Rhai Analyzer is under active development. The current extension is suitable for local use and testing, while language coverage, packaging workflows, and release distribution continue to improve.

This is still a fast-moving MVP release. Large parts of the codebase have not yet gone through the level of detailed review and hardening expected from a mature production extension, so some code paths may still have suboptimal performance characteristics or panic in edge cases.
