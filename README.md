# rhai-analyzer

Workspace skeleton for a future Rhai language server and analysis engine.

## Crates

- `rhai-vfs`: file ids, document versions, and in-memory file contents.
- `rhai-syntax`: parsing-facing syntax types and placeholder parser entry points.
- `rhai-hir`: semantic lowering boundary from syntax into IDE-oriented data.
- `rhai-project`: project and host-environment metadata for Rhai-specific semantics.
- `rhai-db`: analysis host state and immutable snapshots.
- `rhai-ide`: editor-facing queries such as diagnostics, hover, and symbols.
- `rhai-lsp`: LSP transport layer and server capability wiring.
