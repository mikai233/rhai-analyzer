# rhai-lsp

`rhai-lsp` is the LSP transport layer for `rhai-analyzer`.

It is responsible for turning editor/LSP requests into calls against `rhai-ide`, and for translating semantic results into protocol-shaped responses and notifications.

It does not own parsing, semantic lowering, incremental database logic, or editor-facing semantic policy.

## Implemented Features

### Server Core

- Long-lived server state built around `rhai-ide::AnalysisHost`
- Managed open-document tracking with normalized file paths and document versions
- Full-document sync handling for open/change/close workflows
- Query-support warming for hot files after rebuilds

### Protocol and Capability Wiring

- `initialize` response with server metadata
- Advertised capabilities for:
  - full text document sync
  - goto definition
  - references
  - rename
  - hover
  - document symbols
  - completion
  - code actions

### Handlers

- Diagnostic publication based on semantic and project-aware analysis results
- Close/update behavior that refreshes diagnostics consistently
- Code action translation for source fixes and import-related edits

### Test Coverage

- Focused LSP-layer tests for diagnostics
- Focused LSP-layer tests for code actions

## Current Boundaries

- The crate is intentionally thin and currently exposes only a small set of LSP features.
- Completion resolve, signature help, semantic tokens, inlay hints, folding ranges, and call hierarchy are not yet wired at the protocol layer.
- Sync is currently full-document sync, not incremental edit sync.
- Protocol presentation is still fairly minimal and can grow richer as `rhai-ide` adds more structured metadata.

## Next Steps

- Add protocol wiring for more `rhai-ide` queries such as signature help and richer completion flows
- Support incremental sync if/when it becomes worthwhile for the editor integration story
- Expand protocol output richness for diagnostics, code actions, and future semantic features
