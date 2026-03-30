# Neovim Client

This directory is reserved for the future Neovim frontend for `rhai-analyzer`.

## Intended Design

The planned Neovim integration will remain thin and reuse the shared `rhai-lsp` backend.
Its primary responsibilities are expected to be:

- launching `rhai-lsp --stdio`
- forwarding Neovim-specific configuration
- contributing editor-specific user experience such as setup helpers and defaults

Semantic analysis, diagnostics, formatting, and type inference will continue to live in the
shared Rust crates.

## Status

The Neovim client is not implemented yet. This directory currently serves as the placeholder
for the future frontend structure.
