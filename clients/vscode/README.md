# VSCode Client

This directory contains the Visual Studio Code frontend for `rhai-analyzer`.

The extension is intentionally lightweight. It is responsible for:

- locating or connecting to `rhai-lsp`
- starting and restarting the language client
- forwarding editor configuration to the backend
- contributing VSCode-specific assets such as grammar files, language configuration, and snippets

Language semantics, diagnostics, formatting, and type inference remain in the Rust backend.

## Feature Summary

The current client provides:

- local development discovery for `rhai-lsp` under `target/debug`
- explicit server-path configuration
- `stdio` transport for normal editor use
- TCP transport for local protocol debugging
- restart command integration
- output and trace channels for server diagnostics
- TextMate-based baseline syntax highlighting for Rhai files
- Markdown fenced-code Rhai highlighting
- snippet support for common Rhai constructs
- automatic rebuild hooks for extension-host debugging

## Development Workflow

1. Build the server:

   ```powershell
   cargo build -p rhai-lsp
   ```

2. Install client dependencies and build the extension:

   ```powershell
   cd clients/vscode
   npm install
   npm run build
   ```

3. Open `clients/vscode` in VSCode and press `F5`.

   The launch configuration rebuilds both:

   - `rhai-lsp`
   - the VSCode client bundle

4. Open a `.rhai` file in the Extension Development Host and validate the language features.

## Packaging

To build a local `.vsix` package:

```powershell
cd clients/vscode
npm install
npm run package
```

The packaged extension is written to:

```text
clients/vscode/.artifacts/rhai-analyzer.vsix
```

It can then be installed through the VSCode command:

`Extensions: Install from VSIX...`

## Configuration

The client currently exposes these user-facing settings:

- `rhai.server.path`
  - absolute path to a custom `rhai-lsp` binary
- `rhai.server.transport`
  - `stdio` or `tcp`
- `rhai.server.tcpAddress`
  - TCP endpoint used when transport is `tcp`
- `rhai.server.logLevel`
  - log level passed to `rhai-lsp`
- `rhai.trace.server`
  - VSCode LSP trace level
- `rhai.inlayHints.variables`
- `rhai.inlayHints.parameters`
- `rhai.inlayHints.returnTypes`

## Current Status

This client is an MVP frontend intended to validate the backend in a production-like editor.
It is already suitable for local installation and daily testing, while packaging, release,
and multi-platform distribution workflows will continue to improve over time.
