# JetBrains Client

This directory is reserved for the future JetBrains frontend for `rhai-analyzer`.

## Intended Design

The planned JetBrains integration will act as a thin wrapper around the shared `rhai-lsp`
backend, with IDE-specific packaging and configuration layered on top.

The expected responsibilities are:

- launching and managing the shared `rhai-lsp` process
- mapping JetBrains editor features onto LSP capabilities where appropriate
- providing JetBrains-specific packaging and setup

The underlying language semantics will remain in the shared Rust backend rather than being
reimplemented in the client.

## Status

The JetBrains client is not implemented yet. This directory currently acts as a placeholder
for the future frontend structure.
